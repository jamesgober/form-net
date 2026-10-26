//! The `form!` macro.

/// Declares a struct and its [`FromForm`](crate::FromForm) implementation.
///
/// - The struct is emitted exactly as written (attributes, doc comments,
///   visibility); it may not be generic and must have named fields.
/// - A field's wire name is its Rust name; `= "wire-name"` after the type
///   overrides it. Bracket names such as `tags[]` are plain names, matched
///   byte for byte, never expanded into a structure.
/// - The optional `ignore = [ ... ];` after the struct lists names that are
///   accepted and dropped (the anti-forgery field and `_charset_` are the
///   usual ones). Every other undeclared name is refused.
/// - Field types implement [`FromField`](crate::FromField); see the value
///   rules in `docs/API.md`.
///
/// Matching a name is a chain of byte comparisons the compiler turns into a
/// length switch: no hash map, no allocation beyond what the field types
/// own.
///
/// # Examples
///
/// ```
/// use form_net::FromForm;
///
/// form_net::form! {
///     /// The sign-up form.
///     #[derive(Debug)]
///     pub struct Signup {
///         pub email: String,
///         pub password: form_net::Secret,
///         pub display_name: Option<String> = "display-name",
///         pub age: u8,
///         pub newsletter: bool,
///         pub tags: Vec<String> = "tags[]",
///     }
///     ignore = ["csrf_token", "_charset_"];
/// }
///
/// let s = Signup::from_urlencoded(b"email=a%40b.example&password=pw&age=30&tags[]=x&tags[]=y&csrf_token=t")?;
/// assert_eq!((s.age, s.newsletter, s.tags.len()), (30, false, 2));
/// assert!(s.display_name.is_none());
///
/// let err = Signup::from_urlencoded(b"email=a&password=p&age=300").unwrap_err();
/// assert_eq!(err.code(), "extract.invalid");
/// assert!(Signup::from_urlencoded(b"email=a&password=p&age=1&admin=1").is_err());
/// # Ok::<(), form_net::FormError>(())
/// ```
#[macro_export]
macro_rules! form {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $fvis:vis $field:ident : $ty:ty $(= $wire:literal)?
            ),* $(,)?
        }
        $( ignore = [ $($ignored:literal),* $(,)? ]; )?
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(
                $(#[$fmeta])*
                $fvis $field: $ty,
            )*
        }

        const _: () = {
            #[doc(hidden)]
            #[derive(Default)]
            pub struct __FormNetBuilder {
                $( $field: <$ty as $crate::FromField>::Slot, )*
            }

            impl $crate::FormBuilder for __FormNetBuilder {
                type Output = $name;

                fn text(&mut self, name: &str, value: &str) -> ::core::result::Result<(), $crate::FormError> {
                    $(
                        if name == $crate::__form_wire!($field $(, $wire)?) {
                            return <$ty as $crate::FromField>::accept_text(&mut self.$field, value)
                                .map_err(|e| $crate::__value_error(e, ::core::stringify!($field)));
                        }
                    )*
                    $( $( if name == $ignored { return ::core::result::Result::Ok(()); } )* )?
                    ::core::result::Result::Err($crate::__unknown())
                }

                fn file(&mut self, name: &str, file: $crate::FileValue<'_>) -> ::core::result::Result<(), $crate::FormError> {
                    $(
                        if name == $crate::__form_wire!($field $(, $wire)?) {
                            return <$ty as $crate::FromField>::accept_file(&mut self.$field, file)
                                .map_err(|e| $crate::__value_error(e, ::core::stringify!($field)));
                        }
                    )*
                    $( $( if name == $ignored { return ::core::result::Result::Ok(()); } )* )?
                    let _ = file;
                    ::core::result::Result::Err($crate::__unknown())
                }

                fn finish(self) -> ::core::result::Result<$name, $crate::FormError> {
                    ::core::result::Result::Ok($name {
                        $(
                            $field: <$ty as $crate::FromField>::finish(self.$field)
                                .map_err(|e| $crate::__value_error(e, ::core::stringify!($field)))?,
                        )*
                    })
                }

                fn file_field(&self, name: &str) -> ::core::option::Option<bool> {
                    $( if name == $crate::__form_wire!($field $(, $wire)?) { return ::core::option::Option::Some(true); } )*
                    $( $( if name == $ignored { return ::core::option::Option::Some(false); } )* )?
                    ::core::option::Option::None
                }

                fn finish_all(
                    self,
                    __form_net_errors: &mut $crate::__Vec<$crate::FormError>,
                ) -> ::core::option::Option<$name> {
                    let __form_net_before = __form_net_errors.len();
                    $(
                        let $field = match <$ty as $crate::FromField>::finish(self.$field) {
                            ::core::result::Result::Ok(v) => ::core::option::Option::Some(v),
                            ::core::result::Result::Err(e) => {
                                $crate::__push_field_error(__form_net_errors, e, ::core::stringify!($field));
                                ::core::option::Option::None
                            }
                        };
                    )*
                    if __form_net_errors.len() > __form_net_before {
                        return ::core::option::Option::None;
                    }
                    ::core::option::Option::Some($name { $( $field: $field?, )* })
                }
            }

            impl $crate::FromForm for $name {
                type Builder = __FormNetBuilder;
            }
        };
    };
}

/// The wire name of a `form!` field. Not part of the API.
#[doc(hidden)]
#[macro_export]
macro_rules! __form_wire {
    ($field:ident) => {
        ::core::stringify!($field)
    };
    ($field:ident, $wire:literal) => {
        $wire
    };
}
