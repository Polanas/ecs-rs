#[macro_export]
macro_rules! FromIntoLua {
    //Named variant
    (@return $table:ident $enum_name:ident $var_name:ident { $($field_name:ident: $field_ty:ty),* $(,)?}) => {
        return Ok(
            $enum_name::$var_name {
                $(
                    $field_name: $table.get(stringify!($field_name))?
                ),*
            }
        )
    };
    //Tuple variant
    (@return $table:ident $enum_name:ident $var_name:ident ($($field_ty:ty),*)) => {
        return Ok(
            $enum_name::$var_name {
                $(
                    ${ignore($field_ty)}
                    ${index()}: $table.get(${index()} + 1)?
                ),*
            }
        )
    };
    (@return $table:ident $enum_name:ident $var_name:ident)  => {};
    //Named variant
    (@set $self: ident $table:ident $enum_name:ident $var_name:ident { $($field_name:ident: $field_ty:ty),* $(,)?}) => {
        if let $enum_name::$var_name {
            $($field_name),*
        } = $self {
            $(
                $table.set(stringify!($field_name), $field_name)?;
            )*
        }
    };
    //Tuple variant
    (@set $self:ident $table:ident $enum_name:ident $var_name:ident ($($field_ty:ty),*)) => {
        if let $enum_name::$var_name($(
                ${ignore($field_ty)}
                paste::paste!([<$var_name _ ${index()}>])
        ),+) = $self {
            $(
                ${ignore($field_ty)}
                $table.set(${index()} + 1, paste::paste!([<$var_name _ ${index()}>]))?;
                // $table.set(0,10)?;
            )+
        }
    };
    //Unit variant
    (@set $self:ident $table:ident $enum_name:ident $var_name:ident) => {};
    // VariantName
    (
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident
            $(, $($input:tt)*)?
    ) => (FromIntoLua! {
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // VariantName(...)
    (
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident ( $($tt:tt)* )
            $(, $($input:tt)*)?
    ) => (FromIntoLua! {
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName ($($tt)*)
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // VariantName { ... }
    (
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $($variants:tt)*
        ]
        @parsing
            $( #[$var_meta:meta] )*
            $VariantName:ident { $($tt:tt)* }
            $(, $($input:tt)*)?
    ) => (FromIntoLua! {
        $( #[$meta] )*
        $name
        @variants [
            $($variants)*
            {
                $( #[$var_meta] )*
                $VariantName { $($tt)* }
            }
        ]
        @parsing
            $( $($input)* )?
    });

    // Done parsing, time to generate code:
    (
        $( #[$meta:meta] )*
        $name:ident
        @variants [
            $(
                {
                    $( #[$var_meta:meta] )*
                    $VariantName:ident $($variant_assoc:tt)?
                }
            )*
        ]
        @parsing
            // Nothing left to parse
    ) => (

        $( #[$meta] )*
        pub enum $name {
            $(
                $VariantName $(
                    $variant_assoc
                )? ,
            )*
        }

        impl<'lua> mlua::IntoLua<'lua> for $name {
            #[allow(non_snake_case)]
            fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
                let table = lua.create_table()?;
                let enum_table = lua.create_table()?;
                match &self {
                    $(
                        $name::$VariantName {..} => {
                            table.set(stringify!($VariantName), &enum_table)?;
                            FromIntoLua!(@set self enum_table $name $VariantName $($variant_assoc)?);
                        },
                    )*
                };
                Ok(mlua::Value::Table(table))
            }
        }
        impl<'lua> mlua::FromLua<'lua> for $name {
            #[allow(unused_variables)]
            fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
                match value {
                    mlua::Value::Table(table) => {
                        $(
                            if let Ok(enum_table) = table.get::<_, mlua::Table>(stringify!($VariantName)) {
                                FromIntoLua!(@return enum_table $name $VariantName $($variant_assoc)?);
                            }
                        )*
                        todo!()
                    }
                    other => {
                        let type_name = std::any::type_name_of_val(&other);
                        Err(mlua::Error::FromLuaConversionError {
                            from: type_name,
                            to: std::any::type_name::<$name>(),
                            message: Some(format!("expected table, got {}", type_name)),
                        })
                    }
                }
            }
        }

            );

            // == ENTRY POINT ==
            (
                $( #[$meta:meta] )*
                $vis:vis enum $name:ident {
                    $($tt:tt)*
                }
            ) => (FromIntoLua! {
                $( #[$meta] )*
                $name
                // a sequence of brace-enclosed variants
                @variants []
                // remaining tokens to parse
                @parsing
                    $($tt)*
            });
            // Unit-Struct
            (
                $( #[$meta:meta] )*
            //  ^~~~attributes~~~~^
                $vis:vis struct $name:ident;
            ) => {
                $( #[$meta] )*
                $vis struct $name;

                impl<'lua> mlua::IntoLua<'lua> for $name {
                    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
                        let table = lua.create_table()?;
                        Ok(mlua::Value::Table(table))
                    }
                }

                impl<'lua> mlua::FromLua<'lua> for $name {
                    #[allow(unused_variables)]
                    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
                        match value {
                            mlua::Value::Table(data) => {
                                Ok($name)
                            }
                            other => {
                                let type_name = std::any::type_name_of_val(&other);
                                Err(mlua::Error::FromLuaConversionError {
                                    from: type_name,
                                    to: std::any::type_name::<$name>(),
                                    message: Some(format!("expected table, got {}", type_name)),
                                })
                            }
                        }
                    }
                }
            };

            // Tuple-Struct
            (
                $( #[$meta:meta] )*
            //  ^~~~attributes~~~~^
                $vis:vis struct $name:ident (
                    $(
                        $( #[$field_meta:meta] )*
            //          ^~~~field attributes~~~~^
                        $field_vis:vis $field_ty:ty
            //          ^~~~~~a single field~~~~~~^
                    ),*
                $(,)? );
            ) => {
                $( #[$meta] )*
                $vis struct $name (
                    $(
                        $( #[$field_meta] )*
                        $field_vis $field_ty
                    ),*
                );

                impl<'lua> mlua::IntoLua<'lua> for $name {
                    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
                        let table = lua.create_table()?;
                        $(
                            ${ignore($field_ty)}
                            let field_name = const_format::concatcp!("{}", ${index()} + 1u32);
                            table.raw_set(field_name, self.${index()})?;
                        )*
                        Ok(mlua::Value::Table(table))
                    }
                }

                impl<'lua> mlua::FromLua<'lua> for $name {
                    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
                        match value {
                            mlua::Value::Table(data) => {
                                Ok(
                                    $name(
                                        $(
                                            ${ignore($field_ty)}
                                            data.raw_get(${index()} + 1u32)?
                                        ),+
                                    )
                                )
                            }
                            other => {
                                let type_name = std::any::type_name_of_val(&other);
                                Err(mlua::Error::FromLuaConversionError {
                                    from: type_name,
                                    to: std::any::type_name::<$name>(),
                                    message: Some(format!("expected table, got {}", type_name)),
                                })
                            }
                        }
                    }
                }
            };

            // Named-Struct
            (
                $( #[$meta:meta] )*
            //  ^~~~attributes~~~~^
                $vis:vis struct $name:ident {
                    $(
                        $( #[$field_meta:meta] )*
            //          ^~~~field attributes~~~!^
                        $field_vis:vis $field_name:ident : $field_ty:ty
            //          ^~~~~~~~~~~~~~~~~a single field~~~~~~~~~~~~~~~^
                    ),*
                $(,)? }
            ) => {
                $( #[$meta] )*
                $vis struct $name {
                    $(
                        $( #[$field_meta] )*
                        $field_vis $field_name : $field_ty
                    ),*
                }

                impl<'lua> mlua::IntoLua<'lua> for $name {
                    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
                        let table = lua.create_table()?;
                        $(
                            table.raw_set(stringify!($field_name), self.$field_name)?;
                        )*
                        Ok(mlua::Value::Table(table))
                    }
                }

                impl<'lua> mlua::FromLua<'lua> for $name {
                    #[allow(unused_variables)]
                    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
                        match value {
                            mlua::Value::Table(data) => {
                                Ok(
                                    $name {
                                        $(
                                            $field_name: data.get(stringify!($field_name))?,
                                        )*
                                    }
                                )
                            }
                            other => {
                                let type_name = std::any::type_name_of_val(&other);
                                Err(mlua::Error::FromLuaConversionError {
                                    from: type_name,
                                    to: std::any::type_name::<$name>(),
                                    message: Some(format!("expected table, got {}", type_name)),
                                })
                            }
                        }
                    }
                }
    };
}
