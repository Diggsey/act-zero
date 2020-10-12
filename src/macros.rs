#[doc(hidden)]
#[macro_export]
macro_rules! __impl_send {
    (
        @parse $caller:tt receiver=[$($receiver:tt)*] tokens = [. $method:ident ($($args:expr),*)]
    ) => {
        $crate::__impl_send!(@move_args $caller args=[$($args),*] input=[$($receiver)*, $method])
    };
    (
        @parse $caller:tt receiver=[$($receiver:tt)*] tokens = [$token:tt $($tokens:tt)*]
    ) => {
        $crate::__impl_send!(@parse $caller receiver=[$($receiver)* $token] tokens = [$($tokens)*])
    };
    (
        @move_args $caller:tt args = [] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[] moved=[] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0] moved=[arg0] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1] moved=[arg0, arg1] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2] moved=[arg0, arg1, arg2] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3] moved=[arg0, arg1, arg2, arg3] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4] moved=[arg0, arg1, arg2, arg3, arg4] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4, $arg5] moved=[arg0, arg1, arg2, arg3, arg4, arg5] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6] moved=[arg0, arg1, arg2, arg3, arg4, arg5, arg6] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6, $arg7] moved=[arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6, $arg7, $arg8] moved=[arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8] input=$input)
    };
    (
        @move_args $caller:tt args = [$arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr] input = $input:tt
    ) => {
        $crate::__impl_send!(@$caller args=[$arg0, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6, $arg7, $arg8, $arg9] moved=[arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9] input=$input)
    };
    (
        @send args=[$($args:expr),*] moved=[$($moved:ident),*] input=[$addr:expr, $method:ident]
    ) => {
        {
            $(
                let $moved = $args;
            )*
            let addr = $crate::AsAddr::as_addr(&$addr);
            let addr2 = addr.clone();
            $crate::hidden::trace!("send!({}::{}(...))", $crate::hidden::type_name_of_addr(addr).as_display(), stringify!($method));
            $crate::AddrLike::send_mut(addr, Box::new(move |x| {
                $crate::hidden::trace!("{}::{}(...)", $crate::hidden::type_name_of_val(x).as_display(), stringify!($method));
                $crate::hidden::FutureExt::boxed(async move {
                    let _addr = addr2;
                    if let Err(e) = $crate::IntoActorResult::into_actor_result(x.$method($($moved),*).await) {
                        $crate::Actor::error(x, e).await
                    } else {
                        false
                    }
                })
            }));
        }
    };
    (
        @call args=[$($args:expr),*] moved=[$($moved:ident),*] input=[$addr:expr, $method:ident]
    ) => {
        {
            $(
                let $moved = $args;
            )*
            let addr = $crate::AsAddr::as_addr(&$addr);
            let addr2 = addr.clone();
            $crate::hidden::trace!("call!({}::{}(...))", $crate::hidden::type_name_of_addr(addr).as_display(), stringify!($method));
            let (tx, rx) = $crate::hidden::oneshot::channel();
            $crate::AddrLike::send_mut(addr, Box::new(move |x| {
                $crate::hidden::trace!("{}::{}(...)", $crate::hidden::type_name_of_val(x).as_display(), stringify!($method));
                $crate::hidden::FutureExt::boxed(async move {
                    let _addr = addr2;
                    match $crate::IntoActorResult::into_actor_result(x.$method($($moved),*).await) {
                        Ok(x) => {
                            let _ = tx.send(x);
                            false
                        }
                        Err(e) => $crate::Actor::error(x, e).await,
                    }
                })
            }));
            $crate::Produces::Deferred(rx)
        }
    };
}

/// Sends a method call to be executed by the actor.
///
/// ```ignore
/// send!(addr.method(arg1, arg2))
/// ```
///
/// Constraints:
/// - The method must be an inherent method or trait method callable on the
///   actor type.
/// - The method must take `&mut self` as the receiver.
/// - The method must return a future, with an output that implements `IntoActorResult`.
/// - The arguments must be `Send + 'static`.
#[macro_export]
macro_rules! send {
    ($($tokens:tt)*) => {
        $crate::__impl_send!(@parse send receiver=[] tokens=[$($tokens)*])
    };
}

/// Sends a method call to be executed by the actor, and returns a future that can
/// be awaited to get back the result.
///
/// ```ignore
/// call!(addr.method(arg1, arg2))
/// ```
///
/// The same constraints as for the `send!(...)` macro apply.
#[macro_export]
macro_rules! call {
    ($($tokens:tt)*) => {
        $crate::__impl_send!(@parse call receiver=[] tokens=[$($tokens)*])
    };
}

/// Converts an `Addr<T>` or `WeakAddr<T>` to an `Addr<dyn Trait>` or `WeakAddr<dyn Trait>`.
///
/// ```ignore
/// let trait_addr: Addr<dyn Trait> = upcast!(addr);
/// ```
#[macro_export]
macro_rules! upcast {
    ($x:expr) => {
        ($x).upcast(|x| x as _)
    };
}
