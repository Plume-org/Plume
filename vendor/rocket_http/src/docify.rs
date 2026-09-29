macro_rules! docify {
    ([$($doc:tt)*]; $($tt:tt)*) => {
        docify!([$($doc)*] [] $($tt)*);
    };

    // FIXME: Treat $a just like everywhere else. What if we start with @[]?
    ([$a:tt $($b:tt)*] [] $($tt:tt)*) => {
        docify!([$($b)*] [stringify!($a), " "] $($tt)*);
    };

    ([@fence @$lang:tt $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, "\n\n```", stringify!($lang), "\n"] $($tt)*);
    };

    ([@fence $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, "\n\n```\n"] $($tt)*);
    };

    ([@{$($a:tt),*}! $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, $($a),*] $($tt)*);
    };

    ([@{$($a:tt),*} $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, $($a),*, " "] $($tt)*);
    };

    ([@code{$($a:tt)+}! $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, "`", $(stringify!($a)),*, "`"] $($tt)*);
    };

    ([@code{$($a:tt)+} $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, "`", $(stringify!($a)),*, "` "] $($tt)*);
    };

    ([@[$($a:tt)*]! $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, $(stringify!($a)),*] $($tt)*);
    };

    ([@[$($a:tt)*] $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, $(stringify!($a)),*, " "] $($tt)*);
    };

    ([@nl $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, "\n"] $($tt)*);
    };

    (@punct [$a:tt $p:tt $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, stringify!($a), stringify!($p), " "] $($tt)*);
    };

    (@upunct [$a:tt $p:tt $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, stringify!($a), stringify!($p)] $($tt)*);
    };

    ([$a:tt . $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a . $($b)*] $($rest)+); };
    ([$a:tt , $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a , $($b)*] $($rest)+); };
    ([$a:tt ; $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a ; $($b)*] $($rest)+); };
    ([$a:tt : $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a : $($b)*] $($rest)+); };
    ([$a:tt ! $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a ! $($b)*] $($rest)+); };
    ([$a:tt ! $($b:tt)*] $($rest:tt)+) => { docify!(@punct [$a ! $($b)*] $($rest)+); };

    ([$a:tt :: $($b:tt)*] $($rest:tt)+) => { docify!(@upunct [$a :: $($b)*] $($rest)+); };

    ([$a:tt $($b:tt)*] [$($c:tt)+] $($tt:tt)*) => {
        docify!([$($b)*] [$($c)+, stringify!($a), " "] $($tt)*);
    };

    ([] [$($doc:expr),*] $($tt:tt)*) => {
        docify!(concat!($($doc),*), $($tt)*);
    };

    ($x:expr, $($tt:tt)*) => {
        #[doc = $x]
        $($tt)*
    };
}
