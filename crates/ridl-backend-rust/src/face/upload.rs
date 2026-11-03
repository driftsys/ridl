//! Command parameter streams: paired caller values and provider consumers.
use super::{Call, call_doc, future_type, ident, send_fn, ty};
use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn method(call: &Call) -> TokenStream {
    let method = &call.member.method;
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let future = future_type(call);
    let sink = super::call_type_ident(call.member.camel, "Sink");
    let send = send_fn(call);
    let descriptor = &call.member.descriptor;
    let storage = (!call.raw_stream()).then(|| {
        let buffer = buffer(call, "P");
        quote! { buffer: #buffer, held:None, }
    });
    let doc = call_doc(
        call,
        "Sends the command and returns its acknowledgment future and parameter sink. The values share this client's port until both are dropped. The acknowledgment reports delivery, independently of later stream refusal. Dropping an unfinished sink interrupts the stream. An empty sink is locally ready; peer refusal is read from the next push or end.",
    );
    quote! {
        #doc
        pub fn #method(&mut self, #arg: #path) -> (#future<'_,P>, #sink<'_,P>) {
            let __arg = #arg;
            self.upload = UploadState::new();
            let deadline = <#descriptor as ::ridl_rt::contract::Interaction>::MEMBER.call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(self.port.borrow().now().0.saturating_add(max.0)));
            let mut pending = None;
            match #send(&mut *self.port.borrow_mut(), &__arg) {
                Ok(c) => self.upload.correlation.set(Some(c.0)),
                Err(::ridl_rt::port::SendError::Busy) => pending=Some(__arg),
                Err(e) => {self.upload.error.set(Some(::ridl_rt::error::ClientError::Send(e))); self.upload.call_error.set(self.upload.error.get());},
            }
            (#future {port: &self.port, state: &self.upload, pending, deadline, done:false},
             #sink {port: &self.port, state: &self.upload, #storage})
        }
    }
}
fn buffer(call: &Call, port: &str) -> TokenStream {
    if call.raw_stream() {
        let n = proc_macro2::Literal::usize_unsuffixed(call.limit);
        quote! {[0u8;#n]}
    } else {
        super::payload_buffer(call.input.unwrap_or_default(), &ident(port))
    }
}
pub(super) fn provider(call: &Call) -> TokenStream {
    let method = &call.member.method;
    let source = ident(call.member.camel);
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let bound = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        let element = ty(call.input.unwrap_or_default());
        quote! {::ridl_rt::face::Consumer<Item=#element>}
    };
    let doc = call_doc(
        call,
        "Returns the consumer driven by serve after argument validation and successful delivery acknowledgment. Stream failures are reported to the consumer and the producer without changing that acknowledgment.",
    );
    quote! { #[doc="The consumer of this command's parameter stream."] type #source: #bound; #doc fn #method(&mut self,#arg: &#path)->Self::#source; }
}
pub(super) fn state() -> TokenStream {
    quote! {
        struct UploadState {
            correlation: ::core::cell::Cell<Option<::ridl_rt::port::Correlation>>,
            ack: ::core::cell::Cell<bool>,
            call_error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
            ended: ::core::cell::Cell<bool>,
            forgotten: ::core::cell::Cell<bool>,
            error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
        }
        impl UploadState {
            fn new()->Self { Self {call_error: ::core::cell::Cell::new(None), correlation: ::core::cell::Cell::new(None), ack: ::core::cell::Cell::new(false), ended: ::core::cell::Cell::new(false), forgotten: ::core::cell::Cell::new(false), error: ::core::cell::Cell::new(None)} }
            fn forget<P: ::ridl_rt::port::Caller>(&self,port:&::core::cell::RefCell<P>) {
                match self.correlation.get() {Some(c) if !self.forgotten.replace(true)=>port.borrow_mut().forget(c),_=>{}}
            }
        }
        fn upload_push_error(error: ::ridl_rt::port::PushError)->::ridl_rt::error::ClientError {
            ::ridl_rt::error::ClientError::Call(match error {
                ::ridl_rt::port::PushError::Failed(e)=>e,
                _=>::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted),
            })
        }
    }
}
pub(super) fn items(call: &Call, encoding: &TokenStream) -> TokenStream {
    let future = future_type(call);
    let sink = super::call_type_ident(call.member.camel, "Sink");
    let path = ty(call.arg_type);
    let send = send_fn(call);
    let read = super::read_fn(call);
    let correlation = super::correlation_type(call);
    let bounds = quote! {::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable};
    let storage = (!call.raw_stream()).then(|| {let element=ty(call.input.unwrap_or_default()); quote! { buffer:<#element as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer, held:Option<(usize,usize)>, } });
    let element_bound = if call.raw_stream() {
        TokenStream::new()
    } else {
        let element = ty(call.input.unwrap_or_default());
        quote! {#element: ::ridl_rt::payload::Payload<P::Encoding>,}
    };
    let flush = if call.raw_stream() {
        quote! { let _ = cx; ::core::task::Poll::Ready(Ok(())) }
    } else {
        quote! {
                    let Some((start,end))=self.held else {return ::core::task::Poll::Ready(Ok(()));};
                    let Some(c)=self.state.correlation.get() else {self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot,cx.waker());return ::core::task::Poll::Pending;};
                    self.port.borrow().wake_on(::ridl_rt::port::Interest::Stream(c,::ridl_rt::contract::StreamNo(1)),cx.waker());
                    let bytes=&::core::convert::AsRef::<[u8]>::as_ref(&self.buffer)[start..end];
                    let pushed=self.port.borrow_mut().push(c,::ridl_rt::contract::StreamNo(1),bytes);
                    match pushed {
                        Ok(n) if n==bytes.len()=>{self.held=None;::core::task::Poll::Ready(Ok(()))},
                        Ok(_)=>::core::task::Poll::Pending,
                        Err(e)=>{let error=upload_push_error(e);self.state.error.set(Some(error));::core::task::Poll::Ready(Err(error))},
                    }
        }
    };
    let write = write_methods(call);
    let limit = call.raw_stream().then(|| {
        let n = proc_macro2::Literal::usize_unsuffixed(call.limit);
        quote! {#[doc="The maximum chunk length in bytes."] pub const LIMIT:usize=#n;}
    });
    quote! {
        #[doc="The delivery acknowledgment of a command with a parameter stream. Dropping it before acknowledgment forgets the call. Completion leaves an open sink usable."]
        #[must_use]
        pub struct #future<'a,P:#bounds> {
            port:&'a ::core::cell::RefCell<P>, state:&'a UploadState,
            pending:Option<#path>, deadline:Option<::ridl_rt::sample::Timestamp>, done:bool,
        }
        impl<P:#bounds> ::core::marker::Unpin for #future<'_,P> {}
        impl<P:#bounds> ::core::future::Future for #future<'_,P> where #encoding {
            type Output=Result<(),::ridl_rt::error::ClientError>;
            fn poll(self: ::core::pin::Pin<&mut Self>,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Self::Output> {
                let this=self.get_mut(); assert!(!this.done,"call polled after completion");
                if this.pending.is_some() && this.deadline.is_some_and(|deadline| this.port.borrow().now()>deadline) {
                    let e=::ridl_rt::error::ClientError::Send(::ridl_rt::port::SendError::Busy);
                    this.done=true;this.state.call_error.set(Some(e));this.state.error.set(Some(e));return ::core::task::Poll::Ready(Err(e));
                }
                if let Some(arg)=this.pending.take() {
                    this.port.borrow().wake_on(::ridl_rt::port::Interest::Slot,cx.waker());
                    match #send(&mut *this.port.borrow_mut(),&arg) {
                        Ok(c)=>this.state.correlation.set(Some(c.0)),
                        Err(::ridl_rt::port::SendError::Busy)=>this.pending=Some(arg),
                        Err(e)=>{this.state.call_error.set(Some(::ridl_rt::error::ClientError::Send(e)));this.state.error.set(this.state.call_error.get());},
                    }
                }
                let mut result=this.state.call_error.get().map(Err);
                if let (true,Some(c)) = (result.is_none(),this.state.correlation.get()) {
                        this.port.borrow().wake_on(::ridl_rt::port::Interest::Outcome(c),cx.waker());
                        result=#read(&mut *this.port.borrow_mut(),#correlation(c)).map(|v|v.map_err(::ridl_rt::error::ClientError::Call));
                }
                if result.is_none() && this.deadline.is_some_and(|deadline|this.port.borrow().now()>deadline) {
                    result=Some(Err(if this.pending.is_some() {::ridl_rt::error::ClientError::Send(::ridl_rt::port::SendError::Busy)} else {::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Undelivered))}));
                }
                if let Some(result)=result {
                    this.done=true; this.state.ack.set(true);
                    if result.is_err() {this.state.error.set(result.err());this.state.forget(this.port);}
                    else if this.state.ended.get(){this.state.forget(this.port);}
                    ::core::task::Poll::Ready(result)
                } else {::core::task::Poll::Pending}
            }
        }
        impl<P:#bounds> ::core::ops::Drop for #future<'_,P> {
            fn drop(&mut self) {if !self.done {self.state.error.set(Some(::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted))));self.state.forget(self.port);}}
        }
        #[doc="The command's parameter sink. It shares the call's port without allocating. Empty writes are local no-ops; peer refusal is learned from a push or end. Dropping it before end interrupts the upload."]
        pub struct #sink<'a,P:#bounds> where #element_bound {
            port:&'a ::core::cell::RefCell<P>, state:&'a UploadState, #storage
        }
        impl<P:#bounds> #sink<'_,P> where #encoding {
            #limit
            #write
            #[doc="Flushes a held element. With no held element readiness describes local capacity and does not probe the peer."]
            pub fn poll_ready(&mut self,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Result<(),::ridl_rt::error::ClientError>> {
                if let Some(e)=self.state.error.get(){return ::core::task::Poll::Ready(Err(e));}
                #flush
            }
            #[doc="Waits until the sink has no held element."]
            pub async fn ready(&mut self)->Result<(),::ridl_rt::error::ClientError> {::core::future::poll_fn(|cx|self.poll_ready(cx)).await}
            #[doc="Flushes the held element and ends the parameter stream. The call retains ownership until acknowledgment."]
            pub fn poll_end(&mut self,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Result<(),::ridl_rt::error::ClientError>> {
                if self.state.ended.get(){return ::core::task::Poll::Ready(Ok(()));}
                match self.poll_ready(cx) {::core::task::Poll::Ready(Ok(()))=>{},other=>return other}
                let Some(c)=self.state.correlation.get() else {self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot,cx.waker());return ::core::task::Poll::Pending;};
                let result=self.port.borrow_mut().end(c,::ridl_rt::contract::StreamNo(1)).map_err(upload_push_error);
                if result.is_ok(){self.state.ended.set(true);if self.state.ack.get(){self.state.forget(self.port);}}
                else {self.state.error.set(result.err());}
                ::core::task::Poll::Ready(result)
            }
            #[doc="Ends the stream after flushing any held element."]
            pub async fn end(&mut self)->Result<(),::ridl_rt::error::ClientError> {::core::future::poll_fn(|cx|self.poll_end(cx)).await}
        }
        impl<P:#bounds> ::core::ops::Drop for #sink<'_,P> where #element_bound {
            fn drop(&mut self) {
                if !self.state.ended.get() {
                    if let (true,Some(c))=(self.state.error.get().is_none() && !self.state.forgotten.get(), self.state.correlation.get()) {let _=self.port.borrow_mut().abort(c,::ridl_rt::contract::StreamNo(1),::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted));}
                    self.state.forget(self.port);
                }
            }
        }
    }
}
fn write_methods(call: &Call) -> TokenStream {
    if call.raw_stream() {
        let text = call.stream_form == Some(ridl_ir::codegen::v1::StreamForm::Text);
        let (method, wrapper, input, length, bytes) = if text {
            (
                ident("poll_write_str"),
                ident("write_str"),
                quote! {&str},
                quote! {let mut n=input.len().min(Self::LIMIT);while !input.is_char_boundary(n){n-=1;}},
                quote! {&input.as_bytes()[..n]},
            )
        } else {
            (
                ident("poll_write"),
                ident("write"),
                quote! {&[u8]},
                quote! {let n=input.len().min(Self::LIMIT);},
                quote! {&input[..n]},
            )
        };
        return quote! {
            #[doc="Offers a chunk up to LIMIT. Empty input returns zero without a port call. Pending input registers the stream wake interest before trying the push."]
            pub fn #method(&mut self,cx:&mut ::core::task::Context<'_>,input:#input)->::core::task::Poll<Result<usize,::ridl_rt::error::ClientError>> {
                if input.is_empty(){return ::core::task::Poll::Ready(Ok(0));}
                if let Some(e)=self.state.error.get(){return ::core::task::Poll::Ready(Err(e));}
                if self.state.ended.get(){return ::core::task::Poll::Ready(Err(upload_push_error(::ridl_rt::port::PushError::Ended)));}
                let Some(c)=self.state.correlation.get() else {self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot,cx.waker());return ::core::task::Poll::Pending;};
                #length
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Stream(c,::ridl_rt::contract::StreamNo(1)),cx.waker());
                let result=self.port.borrow_mut().push(c,::ridl_rt::contract::StreamNo(1),#bytes);
                match result {
                    Ok(0)=>::core::task::Poll::Pending,
                    Ok(n)=>::core::task::Poll::Ready(Ok(n)),
                    Err(e)=>{let error=upload_push_error(e);self.state.error.set(Some(error));::core::task::Poll::Ready(Err(error))},
                }
            }
            #[doc="Waits until a prefix of the input is accepted and returns its byte length."]
            pub async fn #wrapper(&mut self,input:#input)->Result<usize,::ridl_rt::error::ClientError> {::core::future::poll_fn(|cx|self.#method(cx,input)).await}
        };
    }
    let element = ty(call.input.unwrap_or_default());
    let failures = super::encode_failure_arms("a parameter element", "its buffer");
    quote! {
        #[doc="Encodes and verifies one element. Holds at most one element while the window is full; another send then reports Busy. A zero-byte encoding is an accepted element, sent exactly once."]
        pub fn send(&mut self,item:&#element)->Result<(),::ridl_rt::error::ClientError> {
            if let Some(e)=self.state.error.get(){return Err(e);}
            if self.held.is_some(){return Err(::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Busy)));}
            if self.state.ended.get(){return Err(upload_push_error(::ridl_rt::port::PushError::Ended));}
            let buf=::core::convert::AsMut::<[u8]>::as_mut(&mut self.buffer);
            let base=buf.as_ptr() as usize;
            let checked=(|| {
                let bytes=match ::ridl_rt::payload::Ref::<#element,P::Encoding>::encode(item,buf) {
                    Ok(encoded)=>encoded.bytes(),
                    Err(::ridl_rt::payload::EncodeError::Contract(v))=>return Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(v))),
                    #failures
                };
                match ::ridl_rt::payload::Ref::<#element,P::Encoding>::verify(bytes) {
                    Ok(_)=>Ok((bytes.as_ptr() as usize-base,bytes.len())),
                    Err(::ridl_rt::payload::VerifyError::Contract(v))=>Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(v))),
                    _=>Err(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt)),
                }
            })();
            let (start,len)=match checked {Ok(range)=>range,Err(e)=>{
                if let Some(c)=self.state.correlation.get(){let _=self.port.borrow_mut().abort(c,::ridl_rt::contract::StreamNo(1),e);}
                let error=::ridl_rt::error::ClientError::Call(e);self.state.error.set(Some(error));return Err(error);
            }};
            self.held=Some((start,start+len));
            if let Some(c)=self.state.correlation.get() {
                let bytes=&::core::convert::AsRef::<[u8]>::as_ref(&self.buffer)[start..start+len];
                match self.port.borrow_mut().push(c,::ridl_rt::contract::StreamNo(1),bytes) {
                    Ok(n) if n==len=>self.held=None,
                    Ok(_)=>{},
                    Err(e)=>{let error=upload_push_error(e);self.state.error.set(Some(error));return Err(error);},
                }
            }
            Ok(())
        }
        #[doc="Waits for readiness, then sends one checked element."]
        pub async fn feed(&mut self,item:&#element)->Result<(),::ridl_rt::error::ClientError> {self.ready().await?;self.send(item)}
    }
}

pub(super) fn slot_type(call: &Call) -> TokenStream {
    let source = ident(call.member.camel);
    let storage = if call.raw_stream() {
        let n = proc_macro2::Literal::usize_unsuffixed(call.limit);
        quote! {[u8;#n]}
    } else {
        let t = ty(call.input.unwrap_or_default());
        quote! {InputBuffer<H,#t>}
    };
    let slot = if call.raw_stream() {
        ident("ByteInputSlot")
    } else {
        ident("InputSlot")
    };
    quote! {Option<#slot<P::#source,#storage>>}
}
pub(super) fn slot_items(calls: &[&Call]) -> TokenStream {
    if calls.is_empty() {
        return TokenStream::new();
    }
    let drivers = calls.iter().map(|call| driver(call));
    let named = calls.iter().any(|call| !call.raw_stream()).then(
        || quote! {type InputBuffer<H,T> = <T as ::ridl_rt::payload::Payload<<H as ::ridl_rt::port::Encoded>::Encoding>>::Buffer;
        struct InputSlot<C,B> {claim: ::ridl_rt::port::ClaimId, consumer:C, buffer:B}},
    );
    let bytes=calls.iter().any(|call| call.raw_stream()).then(||quote! {struct ByteInputSlot<C,B> {claim: ::ridl_rt::port::ClaimId, consumer:C, buffer:B, held:Option<(usize,usize)>}});
    quote! {
        #named
        #bytes
        #(#drivers)*
    }
}
pub(super) fn dispatch(call: &Call) -> TokenStream {
    let ordinal = &call.member.ordinal;
    let method = &call.member.method;
    let descriptor = &call.member.descriptor;
    let decode = super::decode_args(call.arg_type, &ident("H"));
    let buffer = buffer(call, "H");
    let slot = if call.raw_stream() {
        ident("ByteInputSlot")
    } else {
        ident("InputSlot")
    };
    let held = call.raw_stream().then(|| quote! {held:None,});
    quote! {
        #ordinal => {
            match #decode {
                Err(e)=>h.settle(claim.id,Err(e)),
                Ok(__arg)=>match <#descriptor as ::ridl_rt::contract::Command>::require(&__arg) {
                    Err(())=>h.settle(claim.id,Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::PreconditionFailed))),
                    Ok(())=>if streams.#method.is_some(){h.settle(claim.id,Err(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Busy)))}else{
                        let accepted=h.settle(claim.id,Ok(&[]));
                        if accepted.is_ok(){
                            streams.#method=Some(#slot{claim:claim.id,consumer:p.#method(&__arg),buffer:#buffer,#held});
                            streams.__started=true;
                        }
                        accepted
                    }
                }
            }
        }
    }
}
fn driver(call: &Call) -> TokenStream {
    let name = quote::format_ident!("drive_input_{}", call.member.method);
    let slot_type = slot_type(call);
    let bounds = if call.raw_stream() {
        TokenStream::new()
    } else {
        let t = ty(call.input.unwrap_or_default());
        quote! {#t: ::ridl_rt::payload::Payload<H::Encoding>,}
    };
    let consumer = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        quote! {::ridl_rt::face::Consumer}
    };
    let ready = if call.raw_stream() {
        TokenStream::new()
    } else {
        quote! {
            match ::ridl_rt::face::Consumer::poll_ready(&mut stream.consumer,cx) {
                ::core::task::Poll::Pending=>{*slot=Some(stream);return false;},
                ::core::task::Poll::Ready(())=>{},
            }
        }
    };
    let chunk = if call.raw_stream() {
        let verify = if call.stream_form == Some(ridl_ir::codegen::v1::StreamForm::Text) {
            quote! {
                if ::core::str::from_utf8(&stream.buffer[..n]).is_err() {
                    let e=::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(::ridl_rt::payload::Violation{type_name:Some("string"),rule: ::ridl_rt::payload::Rule::Utf8}));
                    let _=h.abort_claim(stream.claim,::ridl_rt::contract::StreamNo(1),e);::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer,e);return false;
                }
            }
        } else {
            TokenStream::new()
        };
        quote! {#verify stream.held=Some((0,n));}
    } else {
        let t = ty(call.input.unwrap_or_default());
        quote! {
            let bytes=&::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[..n];
            let decoded=match ::ridl_rt::payload::Ref::<#t,H::Encoding>::verify(bytes) {
                Ok(v)=>Ok(v.decode()),
                Err(::ridl_rt::payload::VerifyError::Contract(v))=>Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(v))),
                _=>Err(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt)),
            };
            match decoded {
                Ok(item)=>::ridl_rt::face::Consumer::accept(&mut stream.consumer,item),
                Err(e)=>{let _=h.abort_claim(stream.claim,::ridl_rt::contract::StreamNo(1),e);::ridl_rt::face::Consumer::failed(&mut stream.consumer,e);return false;},
            }
        }
    };
    let partial=call.raw_stream().then(||quote!{
        if let Some((start,end))=stream.held {
            if start!=end {
                match ::ridl_rt::face::ByteConsumer::poll_write(&mut stream.consumer,cx,&stream.buffer[start..end]) {
                    ::core::task::Poll::Pending=>{*slot=Some(stream);return false;},
                    ::core::task::Poll::Ready(n)=>{assert!(n>0 && n<=end-start,"ByteConsumer returned invalid length");stream.held=if start+n==end {None}else{Some((start+n,end))};*slot=Some(stream);return true;},
                }
            }
            stream.held=None;
        }
    });
    quote! {
        fn #name<H,P>(h:&mut H,slot:&mut #slot_type,cx:&mut ::core::task::Context<'_>,budget:&mut usize)->bool
        where H: ::ridl_rt::port::Encoded+::ridl_rt::port::StreamHandler+::ridl_rt::port::Wakeable,P:Provider,#bounds {
            if slot.is_none(){return false;} if *budget==0{return true;} *budget-=1;
            let mut stream=slot.take().expect("checked above");
            #partial
            #ready
            h.wake_on(::ridl_rt::port::Interest::ClaimStream(stream.claim,::ridl_rt::contract::StreamNo(1)),cx.waker());
            match h.pull_claim(stream.claim,::ridl_rt::contract::StreamNo(1),::core::convert::AsMut::as_mut(&mut stream.buffer)) {
                Ok(::ridl_rt::port::Pull::Chunk(n))=>{#chunk *slot=Some(stream);true},
                Ok(::ridl_rt::port::Pull::End)=>{#consumer::end(&mut stream.consumer);false},
                Ok(::ridl_rt::port::Pull::Failed(e))=>{#consumer::failed(&mut stream.consumer,e);false},
                Ok(::ridl_rt::port::Pull::Pending)=>{*slot=Some(stream);false},
                Err(_)=>{let e=::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt);let _=h.abort_claim(stream.claim,::ridl_rt::contract::StreamNo(1),e);#consumer::failed(&mut stream.consumer,e);false},
            }
        }
    }
}
