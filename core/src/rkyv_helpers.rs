use ecow::EcoString;
use rkyv::{
	Place, SerializeUnsized,
	rancor::{Fallible, ResultExt, Source},
	ser::{Allocator, Writer},
	string::{ArchivedString, StringResolver},
	vec::{ArchivedVec, VecResolver},
	with::{ArchiveWith, DeserializeWith, SerializeWith}
};

pub struct EcoStringAsBytes;

impl ArchiveWith<EcoString> for EcoStringAsBytes {
	type Archived = ArchivedString;
	type Resolver = StringResolver;

	fn resolve_with(field: &EcoString, resolver: Self::Resolver, out: Place<Self::Archived>) {
		ArchivedString::resolve_from_str(field, resolver, out);
	}
}

impl<S> SerializeWith<EcoString, S> for EcoStringAsBytes
where
	str: SerializeUnsized<S>,
	S: Fallible + ?Sized,
	S::Error: Source
{
	fn serialize_with(field: &EcoString, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
		ArchivedString::serialize_from_str(field, serializer)
	}
}

impl<D> DeserializeWith<ArchivedString, EcoString, D> for EcoStringAsBytes
where
	D: Fallible + ?Sized
{
	fn deserialize_with(field: &ArchivedString, _: &mut D) -> Result<EcoString, D::Error> {
		Ok(EcoString::from(field.as_str()))
	}
}

pub struct AsSerde;

impl<T: serde::Serialize> ArchiveWith<T> for AsSerde {
	type Archived = ArchivedVec<u8>;
	type Resolver = VecResolver;

	fn resolve_with(field: &T, resolver: Self::Resolver, out: Place<Self::Archived>) {
		let bytes = serde_brief::to_vec_with_config(
			field,
			serde_brief::Config {
				use_indices: true,
				..Default::default()
			}
		)
		.expect("Failed to serialise value");

		ArchivedVec::resolve_from_slice(&bytes, resolver, out);
	}
}

impl<S, T: serde::Serialize> SerializeWith<T, S> for AsSerde
where
	S: Fallible + ?Sized + Allocator + Writer,
	S::Error: Source
{
	fn serialize_with(field: &T, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
		let bytes = serde_brief::to_vec_with_config(
			field,
			serde_brief::Config {
				use_indices: true,
				..Default::default()
			}
		)
		.into_error::<S::Error>()
		.trace("in serde serialisation")?;

		ArchivedVec::serialize_from_slice(&bytes, serializer)
	}
}

impl<D, T: serde::de::DeserializeOwned> DeserializeWith<ArchivedVec<u8>, T, D> for AsSerde
where
	D: Fallible + ?Sized,
	D::Error: Source
{
	fn deserialize_with(field: &ArchivedVec<u8>, _: &mut D) -> Result<T, D::Error> {
		serde_brief::from_slice(field)
			.into_error::<D::Error>()
			.trace("in serde deserialisation")
	}
}
