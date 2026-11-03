//! Query uploads, their replies, and shared caller ownership.
use super::{Call, ident, ty};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn provider(call: &Call) -> TokenStream {
    let name = ident(call.member.camel);
    let method = &call.member.method;
    let arg = &call.arg;
    let args = ty(call.arg_type);
    let reply = ty(call.reply_type.unwrap());
    let input = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        let input = ty(call.input.unwrap());
        quote! {::ridl_rt::face::Consumer<Item=#input>}
    };
    if call.duplex() {
        return quote! {
            #[doc="The value that consumes the upload and produces the reply concurrently."]
            type #name: #input + ::ridl_rt::face::Source<Item=#reply>;
            #[doc="Starts the upload consumer and reply source after argument validation."]
            fn #method(&mut self,#arg:&#args)->Self::#name;
        };
    }
    let finish = format_ident!("{}_reply", method);
    quote! {
        #[doc="The consumer of the query's upload."] type #name:#input;
        #[doc="Starts consuming the upload after argument validation."]
        fn #method(&mut self,#arg:&#args)->Self::#name;
        #[doc="Computes the reply after the consumer has received its end. The query's ensure clauses are checked before settlement."]
        fn #finish(&mut self,upload:Self::#name)->#reply;
    }
}

pub(super) fn call_items(call: &Call, encoding: &TokenStream) -> TokenStream {
    let future = super::future_type(call);
    let args = ty(call.arg_type);
    let reply = ty(call.reply_type.unwrap());
    let send = super::send_fn(call);
    let read = super::read_fn(call);
    let corr = super::correlation_type(call);
    let stream = super::stream_type(call);
    let bounds = quote! {::ridl_rt::port::Encoded+::ridl_rt::port::StreamCaller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable};
    let element_bound = call
        .duplex()
        .then(|| quote! {#reply: ::ridl_rt::payload::Payload<P::Encoding>,});
    let buffer_field = call
        .duplex()
        .then(|| quote! {buffer:<#reply as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,});
    let output = if call.duplex() {
        quote! {#stream<'a,P>}
    } else {
        quote! {#reply}
    };
    let response = if call.duplex() {
        quote! {
            let c=correlation;
            let result=this.port.borrow_mut().pull(c,::ridl_rt::port::REPLY_STREAM,::core::convert::AsMut::as_mut(&mut this.buffer));
            match result {
                Ok(::ridl_rt::port::Pull::Pending)=>{},
                Ok(pull)=>{
                    let state=match pull {::ridl_rt::port::Pull::Chunk(n)=>StreamState::First(n),::ridl_rt::port::Pull::End=>StreamState::Done,::ridl_rt::port::Pull::Failed(e)=>{this.state.error.set(Some(::ridl_rt::error::ClientError::Call(e)));StreamState::Refused(e)},_=>unreachable!()};
                    this.done=true;this.state.ack.set(true);
                    return ::core::task::Poll::Ready(Ok(#stream{port:this.port,shared:this.state,correlation:c,buffer: ::core::mem::replace(&mut this.buffer,<#reply as ::ridl_rt::payload::Payload<P::Encoding>>::BUFFER),state}));
                },
                Err(e)=>return this.fail(::ridl_rt::error::ClientError::Read(e)),
            }
        }
    } else {
        quote! {
            let response=#read(&mut *this.port.borrow_mut(),#corr(correlation));
        match response {
                Ok(Some(Ok(value)))=>{this.done=true;this.state.ack.set(true);this.state.forget(this.port);return ::core::task::Poll::Ready(Ok(value));},
                Ok(Some(Err(e)))=>return this.fail(::ridl_rt::error::ClientError::Call(e)),
                Err(e)=>return this.fail(::ridl_rt::error::ClientError::Read(e)),
                Ok(None)=>{},
            }
        }
    };
    let interest = if call.duplex() {
        quote! {::ridl_rt::port::Interest::Stream(correlation,::ridl_rt::port::REPLY_STREAM)}
    } else {
        quote! {::ridl_rt::port::Interest::Outcome(correlation)}
    };
    let stream_items = call.duplex().then(|| shared_stream(call, encoding));
    quote! {
        #[doc="The reply future of a query with an upload. The response deadline begins only when the sink successfully sends its end. Dropping an uncompleted future cancels the call."]
        #[must_use]
        pub struct #future<'a,P:#bounds> where #element_bound {
            port:&'a ::core::cell::RefCell<P>,state:&'a UploadState,pending:Option<#args>,done:bool,#buffer_field
        }
        impl<P:#bounds> ::core::marker::Unpin for #future<'_,P> where #element_bound {}
        impl<'a,P:#bounds> #future<'a,P> where #encoding {
            fn fail(&mut self,error: ::ridl_rt::error::ClientError)->::core::task::Poll<Result<#output,::ridl_rt::error::ClientError>> {
                self.pending=None;self.done=true;self.state.error.set(Some(error));self.state.reply_stream.set(false);self.state.forget(self.port);::core::task::Poll::Ready(Err(error))
            }
        }
        impl<'a,P:#bounds> ::core::future::Future for #future<'a,P> where #encoding {
            type Output=Result<#output,::ridl_rt::error::ClientError>;
            fn poll(self: ::core::pin::Pin<&mut Self>,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Self::Output> {
                let this=self.get_mut();assert!(!this.done,"query polled after completion");
                if let Some(e)=this.state.call_error.get(){return this.fail(e);}
                if let Some(arg)=this.pending.take(){
                    this.port.borrow().wake_on(::ridl_rt::port::Interest::Slot,cx.waker());
                    let sent=#send(&mut *this.port.borrow_mut(),&arg);
                    match sent {Ok(c)=>this.state.correlation.set(Some(c.0)),Err(::ridl_rt::port::SendError::Busy)=>this.pending=Some(arg),Err(e)=>return this.fail(::ridl_rt::error::ClientError::Send(e))}
                }
                if let Some(correlation)=this.state.correlation.get(){
                    this.port.borrow().wake_on(#interest,cx.waker());
                    #response
                }
                if this.state.reply_deadline.get().is_some_and(|deadline|this.port.borrow().now()>deadline){return this.fail(::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Timeout)));}
                ::core::task::Poll::Pending
            }
        }
        impl<P:#bounds> ::core::ops::Drop for #future<'_,P> where #element_bound {
            fn drop(&mut self){if !self.done{self.state.error.set(Some(::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted))));self.state.forget(self.port);}}
        }
        #stream_items
    }
}
fn shared_stream(call: &Call, encoding: &TokenStream) -> TokenStream {
    let stream = super::stream_type(call);
    let reply = ty(call.reply_type.unwrap());
    let bounds = quote! {::ridl_rt::port::Encoded+::ridl_rt::port::StreamCaller+::ridl_rt::port::Clock+::ridl_rt::port::Wakeable};
    quote! {
        #[doc="The query's reply stream, sharing its port and cleanup state with the upload sink. Each chunk is checked before decoding; an error is yielded once. Dropping it cancels the call."]
        pub struct #stream<'a,P:#bounds> where #reply: ::ridl_rt::payload::Payload<P::Encoding> {
            port:&'a ::core::cell::RefCell<P>,shared:&'a UploadState,correlation: ::ridl_rt::port::Correlation,buffer:<#reply as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,state:StreamState,
        }
        impl<P:#bounds> #stream<'_,P> where #encoding {
            #[doc="Pulls one checked reply element, or its terminal outcome. The upload can remain open while replies arrive."]
            pub fn poll_next(&mut self,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Option<Result<#reply,::ridl_rt::error::ClientError>>> {
                let n=match self.state {
                    StreamState::Done|StreamState::Stopped=>{self.shared.reply_stream.set(false);self.shared.forget(self.port);return ::core::task::Poll::Ready(None);},
                    StreamState::Refused(e)=>{self.shared.error.set(Some(::ridl_rt::error::ClientError::Call(e)));self.state=StreamState::Stopped;return ::core::task::Poll::Ready(Some(Err(::ridl_rt::error::ClientError::Call(e))));},
                    StreamState::First(n)=>{self.state=StreamState::Open;n},
                    StreamState::Open=>{
                        self.port.borrow().wake_on(::ridl_rt::port::Interest::Stream(self.correlation,::ridl_rt::port::REPLY_STREAM),cx.waker());
                        let result=self.port.borrow_mut().pull(self.correlation,::ridl_rt::port::REPLY_STREAM,::core::convert::AsMut::as_mut(&mut self.buffer));
                        match result {
                            Ok(::ridl_rt::port::Pull::Pending)=>return ::core::task::Poll::Pending,
                            Ok(::ridl_rt::port::Pull::Chunk(n))=>n,
                            Ok(::ridl_rt::port::Pull::End)=>{self.state=StreamState::Done;self.shared.reply_stream.set(false);self.shared.forget(self.port);return ::core::task::Poll::Ready(None);},
                            Ok(::ridl_rt::port::Pull::Failed(e))=>{self.shared.error.set(Some(::ridl_rt::error::ClientError::Call(e)));self.state=StreamState::Stopped;return ::core::task::Poll::Ready(Some(Err(::ridl_rt::error::ClientError::Call(e))));},
                            Err(e)=>{self.shared.error.set(Some(::ridl_rt::error::ClientError::Read(e)));self.state=StreamState::Stopped;return ::core::task::Poll::Ready(Some(Err(::ridl_rt::error::ClientError::Read(e))));},
                        }
                    }
                };
                let bytes=&::core::convert::AsRef::<[u8]>::as_ref(&self.buffer)[..n];
                let result=match ::ridl_rt::payload::Ref::<#reply,P::Encoding>::verify(bytes){Ok(value)=>Ok(value.decode()),Err(::ridl_rt::payload::VerifyError::Contract(v))=>Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(v))),_=>Err(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt))};
                match result {Ok(value)=>::core::task::Poll::Ready(Some(Ok(value))),Err(e)=>{self.shared.error.set(Some(::ridl_rt::error::ClientError::Call(e)));let _=self.port.borrow_mut().abort(self.correlation,::ridl_rt::port::REPLY_STREAM,e);self.state=StreamState::Stopped;::core::task::Poll::Ready(Some(Err(::ridl_rt::error::ClientError::Call(e))))}}
            }
            #[doc="Waits for the next reply element or terminal outcome."]
            pub async fn next(&mut self)->Option<Result<#reply,::ridl_rt::error::ClientError>>{::core::future::poll_fn(|cx|self.poll_next(cx)).await}
        }
        impl<P:#bounds> ::core::ops::Drop for #stream<'_,P> where #reply: ::ridl_rt::payload::Payload<P::Encoding> {
            fn drop(&mut self){if !self.shared.ended.get() && self.shared.error.get().is_none(){self.shared.error.set(Some(::ridl_rt::error::ClientError::Call(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted))));}self.shared.reply_stream.set(false);self.shared.forget(self.port);}
        }
    }
}

fn slot_name(call: &Call) -> proc_macro2::Ident {
    format_ident!("__Ridl{}Upload", call.member.camel)
}
pub(super) fn slot_type(call: &Call) -> TokenStream {
    let name = slot_name(call);
    quote! {Option<#name<H,P>>}
}
fn receiver(call: &Call) -> TokenStream {
    if call.duplex() {
        quote! {stream.reply.as_mut().expect("active reply").source.inner}
    } else {
        quote! {stream.consumer}
    }
}
fn input_bound(call: &Call) -> TokenStream {
    if call.raw_stream() {
        TokenStream::new()
    } else {
        let t = ty(call.input.unwrap());
        quote! {#t: ::ridl_rt::payload::Payload<H::Encoding>,}
    }
}
fn output_bound(call: &Call) -> TokenStream {
    let t = ty(call.reply_type.unwrap());
    quote! {#t: ::ridl_rt::payload::Payload<H::Encoding>,}
}
pub(super) fn step_enum() -> TokenStream {
    quote! {enum __RidlUploadStep {Pending,Progress,End,Failed(::ridl_rt::error::CallError),Budget}}
}

pub(super) fn slot_items(call: &Call) -> TokenStream {
    let name = slot_name(call);
    let assoc = ident(call.member.camel);
    let args = ty(call.arg_type);
    let reply = ty(call.reply_type.unwrap());
    let input = input_bound(call);
    let output = output_bound(call);
    let storage = if call.raw_stream() {
        let n = proc_macro2::Literal::usize_unsuffixed(call.limit);
        quote! {[u8;#n]}
    } else {
        let t = ty(call.input.unwrap());
        quote! {<#t as ::ridl_rt::payload::Payload<H::Encoding>>::Buffer}
    };
    let held = call
        .raw_stream()
        .then(|| quote! {held:Option<(usize,usize)>,});
    let alias = format_ident!("__Ridl{}Reply", call.member.camel);
    let alias_item=call.duplex().then(||{let wrapper=wrapper_name(call);quote!{type #alias<H,P> = StreamSlot<#wrapper<<P as Provider>::#assoc>,ElementBuffer<H,#reply>>;}});
    let owner = if call.duplex() {
        quote! {reply:Option<#alias<H,P>>,}
    } else {
        quote! {consumer:P::#assoc,args:#args,}
    };
    let slot_output = call.duplex().then(|| output.clone());
    let step = step(call);
    let driver = driver(call);
    let wrapper = call.duplex().then(|| source_wrapper(call));
    quote! {
        #wrapper
        #alias_item
        struct #name<H: ::ridl_rt::port::Encoded,P:Provider> where #input #slot_output {
            claim: ::ridl_rt::port::ClaimId, buffer:#storage,#held #owner __port: ::core::marker::PhantomData<H>,
        }
        #step
        #driver
    }
}
fn wrapper_name(call: &Call) -> proc_macro2::Ident {
    format_ident!("__Ridl{}Source", call.member.camel)
}
fn source_wrapper(call: &Call) -> TokenStream {
    let name = wrapper_name(call);
    let reply = ty(call.reply_type.unwrap());
    let consumer = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        let t = ty(call.input.unwrap());
        quote! {::ridl_rt::face::Consumer<Item=#t>}
    };
    let consumer_trait = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        quote! {::ridl_rt::face::Consumer}
    };
    quote! {
        struct #name<C:#consumer+::ridl_rt::face::Source<Item=#reply>>{inner:C,input_done:bool}
        impl<C:#consumer+::ridl_rt::face::Source<Item=#reply>> ::ridl_rt::face::Source for #name<C>{
            type Item=#reply;
            fn poll_next(&mut self,cx:&mut ::core::task::Context<'_>)->::core::task::Poll<Option<Result<Self::Item,::ridl_rt::face::Interrupted>>>{::ridl_rt::face::Source::poll_next(&mut self.inner,cx)}
        }
        impl<C:#consumer+::ridl_rt::face::Source<Item=#reply>> ::core::ops::Drop for #name<C>{
            fn drop(&mut self){if !self.input_done{#consumer_trait::failed(&mut self.inner,::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Interrupted));}}
        }
    }
}

fn step(call: &Call) -> TokenStream {
    let name = slot_name(call);
    let method = format_ident!("step_upload_{}", call.member.method);
    let input = input_bound(call);
    let output = call.duplex().then(|| output_bound(call));
    let receiver = receiver(call);
    let ready=(!call.raw_stream()).then(||quote!{if ::ridl_rt::face::Consumer::poll_ready(&mut #receiver,cx).is_pending(){return __RidlUploadStep::Pending;}});
    let held=call.raw_stream().then(||quote!{
        if let Some((start,end))=stream.held {
            if start<end {
                return match ::ridl_rt::face::ByteConsumer::poll_write(&mut #receiver,cx,&stream.buffer[start..end]){
                    ::core::task::Poll::Pending=>__RidlUploadStep::Pending,
                    ::core::task::Poll::Ready(n)=>{assert!(n>0 && n<=end-start,"ByteConsumer returned invalid length");stream.held=if start+n==end{None}else{Some((start+n,end))};__RidlUploadStep::Progress},
                };
            }
            stream.held=None;
        }
    });
    let chunk = if call.raw_stream() {
        let text=(call.stream_form==Some(ridl_ir::codegen::v1::StreamForm::Text)).then(||quote!{
            if ::core::str::from_utf8(&stream.buffer[..n]).is_err(){return __RidlUploadStep::Failed(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(::ridl_rt::payload::Violation{type_name:Some("string"),rule: ::ridl_rt::payload::Rule::Utf8})));}
        });
        quote! {#text stream.held=Some((0,n));__RidlUploadStep::Progress}
    } else {
        let t = ty(call.input.unwrap());
        quote! {
            let bytes=&::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[..n];
            match ::ridl_rt::payload::Ref::<#t,H::Encoding>::verify(bytes){
                Ok(value)=>{::ridl_rt::face::Consumer::accept(&mut #receiver,value.decode());__RidlUploadStep::Progress},
                Err(::ridl_rt::payload::VerifyError::Contract(v))=>__RidlUploadStep::Failed(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(v))),
                _=>__RidlUploadStep::Failed(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt)),
            }
        }
    };
    quote! {
        fn #method<H,P>(h:&mut H,stream:&mut #name<H,P>,cx:&mut ::core::task::Context<'_>,budget:&mut usize)->__RidlUploadStep
        where H: ::ridl_rt::port::Encoded+::ridl_rt::port::StreamHandler+::ridl_rt::port::Wakeable,P:Provider,#input #output {
            if *budget==0{return __RidlUploadStep::Budget;}*budget-=1;
            #held
            #ready
            h.wake_on(::ridl_rt::port::Interest::ClaimStream(stream.claim,::ridl_rt::contract::StreamNo(1)),cx.waker());
            match h.pull_claim(stream.claim,::ridl_rt::contract::StreamNo(1),::core::convert::AsMut::as_mut(&mut stream.buffer)){
                Ok(::ridl_rt::port::Pull::Pending)=>__RidlUploadStep::Pending,
                Ok(::ridl_rt::port::Pull::End)=>__RidlUploadStep::End,
                Ok(::ridl_rt::port::Pull::Failed(e))=>__RidlUploadStep::Failed(e),
                Ok(::ridl_rt::port::Pull::Chunk(n))=>{#chunk},
                Err(_)=>__RidlUploadStep::Failed(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Corrupt)),
            }
        }
    }
}
fn driver(call: &Call) -> TokenStream {
    let name = slot_name(call);
    let method = format_ident!("drive_input_{}", call.member.method);
    let step = format_ident!("step_upload_{}", call.member.method);
    let input = input_bound(call);
    let output = output_bound(call);
    let descriptor = &call.member.descriptor;
    let receiver = receiver(call);
    let consumer = if call.raw_stream() {
        quote! {::ridl_rt::face::ByteConsumer}
    } else {
        quote! {::ridl_rt::face::Consumer}
    };
    let failed_result = if call.duplex() {
        quote! {return false;}
    } else {
        quote! {false}
    };
    let failed = quote! {
        let _=h.abort_claim(stream.claim,::ridl_rt::contract::StreamNo(1),e);
        #consumer::failed(&mut #receiver,e);
        let _=h.settle(stream.claim,Err(e));
        #failed_result
    };
    let assoc = ident(call.member.camel);
    let function = if call.duplex() {
        let output_ty = ty(call.reply_type.unwrap());
        let wrapper = wrapper_name(call);
        quote! {
            let step=if stream.reply.as_ref().expect("active reply").source.input_done {__RidlUploadStep::Pending}else{#step(h,&mut stream,cx,budget)};
            let again=match step {
                __RidlUploadStep::End=>{#consumer::end(&mut #receiver);stream.reply.as_mut().expect("active reply").source.input_done=true;true},
                __RidlUploadStep::Failed(e)=>{stream.reply.as_mut().expect("active reply").source.input_done=true;#failed},
                __RidlUploadStep::Progress|__RidlUploadStep::Budget=>true,
                __RidlUploadStep::Pending=>false,
            };
            let output_again=drive_stream::<H,#output_ty,#wrapper<P::#assoc>> (h,&mut stream.reply,cx,budget);
            if stream.reply.is_some(){*slot=Some(stream);again||output_again}else{false}
        }
    } else {
        let finish = format_ident!("{}_reply", call.member.method);
        let buffer =
            super::buffer_prologue(super::payload_buffer(call.reply_type.unwrap(), &ident("H")));
        let encode = super::encode_into_settled(
            call.reply_type.unwrap(),
            quote! {&reply},
            quote! {buf},
            "the reply buffer",
            &ident("H"),
            quote! {h.settle(stream.claim,Ok(bytes))},
            quote! {h.settle(stream.claim,Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::InvalidValue(__violation))))},
        );
        quote! {
            match #step(h,&mut stream,cx,budget){
                __RidlUploadStep::Pending=>{*slot=Some(stream);false},
                __RidlUploadStep::Progress|__RidlUploadStep::Budget=>{*slot=Some(stream);true},
                __RidlUploadStep::Failed(e)=>{#failed},
                __RidlUploadStep::End=>{
                    #consumer::end(&mut stream.consumer);
                    let reply=p.#finish(stream.consumer);
                    #buffer
                    let _=match <#descriptor as ::ridl_rt::contract::Query>::ensure(&stream.args,&reply){
                        Ok(())=>#encode,
                        Err(())=>h.settle(stream.claim,Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::ContractBroken))),
                    };
                    false
                },
            }
        }
    };
    let provider_arg = if call.duplex() {
        quote! {_p:&mut P}
    } else {
        quote! {p:&mut P}
    };
    let allow = super::allow_deprecated(std::iter::once(&call.member));
    quote! {
        #allow
        fn #method<H,P>(h:&mut H,#provider_arg,slot:&mut Option<#name<H,P>>,cx:&mut ::core::task::Context<'_>,budget:&mut usize)->bool
        where H: ::ridl_rt::port::Encoded+::ridl_rt::port::StreamHandler+::ridl_rt::port::Wakeable,P:Provider,#input #output {
            let Some(mut stream)=slot.take()else{return false;};
            #function
        }
    }
}

pub(super) fn dispatch(call: &Call) -> TokenStream {
    let name = slot_name(call);
    let ordinal = &call.member.ordinal;
    let method = &call.member.method;
    let decode = super::decode_args(call.arg_type, &ident("H"));
    let descriptor = &call.member.descriptor;
    let buffer = if call.raw_stream() {
        let n = proc_macro2::Literal::usize_unsuffixed(call.limit);
        quote! {[0u8;#n]}
    } else {
        super::payload_buffer(call.input.unwrap(), &ident("H"))
    };
    let held = call.raw_stream().then(|| quote! {held:None,});
    let owner = if call.duplex() {
        let wrapper = wrapper_name(call);
        let out = super::payload_buffer(call.reply_type.unwrap(), &ident("H"));
        quote! {
            reply:Some(StreamSlot{claim:claim.id,source:#wrapper{inner:p.#method(&__arg),input_done:false},buffer:#out,held:None}),
        }
    } else {
        quote! {consumer:p.#method(&__arg),args:__arg,}
    };
    quote! {
        #ordinal=>match #decode {
            Err(e)=>h.settle(claim.id,Err(e)),
            Ok(__arg)=>match <#descriptor as ::ridl_rt::contract::Query>::require(&__arg){
                Err(())=>h.settle(claim.id,Err(::ridl_rt::error::CallError::Contract(::ridl_rt::error::Contract::PreconditionFailed))),
                Ok(())=>if streams.#method.is_some(){h.settle(claim.id,Err(::ridl_rt::error::CallError::Transport(::ridl_rt::error::Transport::Busy)))}else{
                    streams.#method=Some(#name{claim:claim.id,buffer:#buffer,#held #owner __port: ::core::marker::PhantomData});streams.__started=true;Ok(())
                }
            }
        },
    }
}
