// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::types::{Namespace, TypeName};
use proc_macro2::TokenStream;
use std::collections::HashSet;
use syn::{ForeignItemFn, Ident, ImplItem, Item, ItemConst, ItemType, ItemUse};

use super::{codegen_cpp::AdditionalNeed, parse::type_converter::TypeConverter};

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum TypeKind {
    Pod,                // trivial. Can be moved and copied in Rust.
    NonPod, // has destructor or non-trivial move constructors. Can only hold by UniquePtr
    ForwardDeclaration, // no full C++ declaration available - can't even generate UniquePtr
}

/// Whether and how this type should be exposed in the mods constructed
/// for actual end-user use.
#[derive(Clone)]
pub(crate) enum Use {
    /// Not used
    Unused,
    /// Uses from cxx::bridge
    Used,
    /// 'use' points to cxx::bridge with a different name
    UsedWithAlias(Ident),
    /// 'use' directive points to bindgen
    UsedFromBindgen,
}

/// Common details for types of API which are a type and will require
/// us to generate an ExternType.
pub(crate) struct TypeApiDetails {
    pub(crate) fulltypath: Vec<Ident>,
    pub(crate) final_ident: Ident,
    pub(crate) tynamestring: String,
}

/// An entry which needs to go into an `impl` block for a given type.
pub(crate) struct ImplBlockDetails {
    pub(crate) item: ImplItem,
    pub(crate) ty: Ident,
}
/// A ForeignItemFn with a little bit of context about the
/// type which is most likely to be 'this'
#[derive(Clone)]
pub(crate) struct FuncToConvert {
    pub(crate) item: ForeignItemFn,
    pub(crate) virtual_this_type: Option<TypeName>,
    pub(crate) self_ty: Option<TypeName>,
}

/// Layers of analysis which may be applied to decorate each API.
/// See description of the purpose of this trait within `Api`.
pub(crate) trait ApiAnalysis {
    type TypeAnalysis;
    type FunAnalysis;
}

/// No analysis has been applied to this API.
pub(crate) struct NullAnalysis;

impl ApiAnalysis for NullAnalysis {
    type TypeAnalysis = ();
    type FunAnalysis = ();
}

pub(crate) enum TypedefKind {
    Type(ItemType),
    Use(ItemUse),
}

/// Different types of API we might encounter.
pub(crate) enum ApiDetail<T: ApiAnalysis> {
    /// A synthetic type we've manufactured in order to
    /// concretize some templated C++ type.
    ConcreteType {
        ty_details: TypeApiDetails,
        additional_cpp: AdditionalNeed,
    },
    /// A simple note that we want to make a constructor for
    /// a `std::string` on the heap.
    StringConstructor,
    /// A function. May include some analysis.
    Function {
        fun: FuncToConvert,
        analysis: T::FunAnalysis,
    },
    /// A constant.
    Const { const_item: ItemConst },
    /// A typedef found in the bindgen output which we wish
    /// to pass on in our output
    Typedef { payload: TypedefKind },
    /// A type (struct or enum) encountered in the
    /// `bindgen` output.
    Type {
        ty_details: TypeApiDetails,
        for_extern_c_ts: TokenStream,
        is_forward_declaration: bool,
        bindgen_mod_item: Option<Item>,
        analysis: T::TypeAnalysis,
    },
    /// A variable-length C integer type (e.g. int, unsigned long).
    CType { typename: TypeName },
    /// A typedef which doesn't point to any actual useful kind of
    /// type, but instead to something which `bindgen` couldn't figure out
    /// and has therefore itself made opaque and mysterious.
    OpaqueTypedef,
}

/// Any API we encounter in the input bindgen rs which we might want to pass
/// onto the output Rust or C++.
///
/// This type is parameterized over an `ApiAnalysis`. This is any additional
/// information which we wish to apply to our knowledge of our APIs later
/// during analysis phases. It might be a excessively traity to parameterize
/// this type; we might be better off relying on an `Option<SomeKindOfAnalysis>`
/// but for now it's working.
///
/// This is not as high-level as the equivalent types in `cxx` or `bindgen`,
/// because sometimes we pass on the `bindgen` output directly in the
/// Rust codegen output.
pub(crate) struct Api<T: ApiAnalysis> {
    pub(crate) ns: Namespace,
    pub(crate) id: Ident,
    /// Any dependencies of this API, such that during garbage collection
    /// we can ensure to keep them.
    pub(crate) deps: HashSet<TypeName>,
    /// Details of this specific API kind.
    pub(crate) detail: ApiDetail<T>,
}

pub(crate) type UnanalyzedApi = Api<NullAnalysis>;

impl<T: ApiAnalysis> Api<T> {
    pub(crate) fn typename(&self) -> TypeName {
        TypeName::new(&self.ns, &self.id.to_string())
    }
}

/// Results of parsing the bindgen mod. This is what is passed from
/// the parser to the analysis phases.
pub(crate) struct ParseResults {
    /// All APIs encountered. This is the main thing.
    pub(crate) apis: Vec<UnanalyzedApi>,
    /// A database containing known relationships between types.
    /// In particular, any typedefs detected.
    /// This should probably be replaced by extracting this information
    /// from APIs as necessary later. TODO
    pub(crate) type_converter: TypeConverter,
}
