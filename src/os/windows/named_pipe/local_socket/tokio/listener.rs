use super::Stream;
use crate::{
	local_socket::{traits::tokio as traits, ListenerOptions, NameInner},
	os::windows::named_pipe::{
		pipe_mode,
		tokio::{PipeListener as GenericPipeListener, PipeListenerOptionsExt as _},
		PipeListenerOptions,
	},
	Sealed,
};
use futures_core::{FusedStream as FusedAsyncIterator, Stream as AsyncIterator};
use std::{
	future::Future,
	io,
	pin::{pin, Pin},
	task::{Context, Poll},
};

type PipeListener = GenericPipeListener<pipe_mode::Bytes, pipe_mode::Bytes>;

#[derive(Debug)]
pub struct Listener(PipeListener);
impl Sealed for Listener {}
impl traits::Listener for Listener {
	type Stream = Stream;

	fn from_options(options: ListenerOptions<'_>) -> io::Result<Self> {
		let mut impl_options = PipeListenerOptions::new();
		let NameInner::NamedPipe(path) = options.name.0;
		impl_options.path = path;
		impl_options.security_descriptor = options.security_descriptor;
		impl_options.create_tokio().map(Self)
	}
	async fn accept(&self) -> io::Result<Stream> {
		let inner = self.0.accept().await?;
		Ok(Stream(inner))
	}
	fn do_not_reclaim_name_on_drop(&mut self) {}
}
impl AsyncIterator for Listener {
	type Item = io::Result<Stream>;
	#[inline(always)]
	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		pin!(traits::Listener::accept(self.get_mut()))
			.poll(cx)
			.map(Some)
	}
}
impl FusedAsyncIterator for Listener {
	#[inline(always)]
	fn is_terminated(&self) -> bool {
		false
	}
}
