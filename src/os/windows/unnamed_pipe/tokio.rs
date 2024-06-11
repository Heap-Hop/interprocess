//! Windows-specific functionality for Tokio-based unnamed pipes.

use crate::{
	os::windows::{
		limbo::{
			tokio::{send_off, Corpse},
			LIMBO_ERR, REBURY_ERR,
		},
		unnamed_pipe::CreationOptions,
		winprelude::*,
		TokioFlusher,
	},
	unnamed_pipe::{
		tokio::{Recver as PubRecver, Sender as PubSender},
		Recver as SyncRecver, Sender as SyncSender,
	},
	Sealed, UnpinExt,
};
use std::{
	io,
	pin::Pin,
	task::{ready, Context, Poll},
};
use tokio::{fs::File, io::AsyncWrite};

fn pair2pair((tx, rx): (SyncSender, SyncRecver)) -> io::Result<(PubSender, PubRecver)> {
	Ok((PubSender(tx.try_into()?), PubRecver(rx.try_into()?)))
}

#[inline]
pub(crate) fn pipe_impl() -> io::Result<(PubSender, PubRecver)> {
	pair2pair(super::pipe_impl()?)
}

/// Tokio-specific extensions to [`CreationOptions`].
#[allow(private_bounds)]
pub trait CreationOptionsExt: Sealed {
	/// Creates a Tokio-based unnamed pipe and returns its sending and receiving ends, or an error
	/// if one occurred.
	fn create_tokio(self) -> io::Result<(PubSender, PubRecver)>;
}
impl CreationOptionsExt for CreationOptions<'_> {
	#[inline]
	fn create_tokio(self) -> io::Result<(PubSender, PubRecver)> {
		pair2pair(self.create()?)
	}
}

#[derive(Debug)]
pub(crate) struct Recver(File);
impl TryFrom<SyncRecver> for Recver {
	type Error = io::Error;
	fn try_from(rx: SyncRecver) -> io::Result<Self> {
		Ok(Self(File::from_std(
			<std::fs::File as From<OwnedHandle>>::from(rx.into()),
		)))
	}
}
multimacro! {
	Recver,
	pinproj_for_unpin(File),
	forward_tokio_read,
	forward_as_handle,
}

#[derive(Debug)]
pub(crate) struct Sender {
	io: Option<File>,
	flusher: TokioFlusher,
	needs_flush: bool,
}

impl AsyncWrite for Sender {
	#[inline]
	fn poll_write(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		self.needs_flush = true;
		let rslt = ready!(self.io.as_mut().expect(LIMBO_ERR).pin().poll_write(cx, buf));
		if rslt.is_err() {
			self.needs_flush = false;
		}
		Poll::Ready(rslt)
	}
	#[inline]
	fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
		// Unnamed pipes on Unix can't be flushed
		Poll::Ready(Ok(()))
	}
	#[inline]
	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		// For limbo elision given cooperative downstream
		let slf = self.get_mut();
		slf.flusher.poll_flush_mut(
			slf.io.as_ref().expect(LIMBO_ERR).as_handle(),
			&mut slf.needs_flush,
			cx,
		)
	}
}

impl Drop for Sender {
	fn drop(&mut self) {
		let corpse = Corpse::Unnamed(self.io.take().expect(REBURY_ERR));
		if self.needs_flush {
			send_off(corpse);
		}
	}
}

impl TryFrom<SyncSender> for Sender {
	type Error = io::Error;
	fn try_from(tx: SyncSender) -> io::Result<Self> {
		let handle = OwnedHandle::from(tx);
		Ok(Self {
			io: Some(File::from_std(std::fs::File::from(handle))),
			flusher: TokioFlusher::new(),
			needs_flush: false,
		})
	}
}

impl AsHandle for Sender {
	fn as_handle(&self) -> BorrowedHandle<'_> {
		self.io.as_ref().expect(LIMBO_ERR).as_handle()
	}
}
