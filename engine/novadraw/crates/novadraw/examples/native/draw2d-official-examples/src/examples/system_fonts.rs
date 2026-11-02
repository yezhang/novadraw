#[cfg(test)]
use novadraw::render::ResourceId;
use novadraw::{
    render::{BuiltinFont, FontData},
    runtime::Runtime,
};
use parley::fontique::{
    Attributes, Collection, CollectionOptions, FontStyle, FontWeight, FontWidth, GenericFamily,
    QueryFamily, QueryStatus, SourceCache,
};

pub(super) const LOGICAL_PIXELS_PER_POINT: f32 = 96.0 / 72.0;
pub(super) const NORMAL_FONT_WEIGHT: f32 = 400.0;
pub(super) const BOLD_FONT_WEIGHT: f32 = 700.0;

#[derive(Clone, Copy)]
pub(super) struct SystemFontRequest {
    pub family: &'static str,
    pub weight: f32,
    pub fallback: BuiltinFont,
}

#[derive(Clone, Debug)]
pub(super) struct RegisteredHostFont {
    pub family: String,
    #[cfg(test)]
    pub resource_id: ResourceId,
    #[cfg(test)]
    pub system: bool,
}

struct ResolvedSystemFont {
    family: String,
    bytes: Vec<u8>,
}

struct SystemFontCatalog {
    collection: Collection,
    source_cache: SourceCache,
}

impl SystemFontCatalog {
    fn new() -> Self {
        Self {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: true,
            }),
            source_cache: SourceCache::default(),
        }
    }

    fn resolve(&mut self, family: &str, weight: f32) -> Option<ResolvedSystemFont> {
        let family_id = self.collection.family_id(family)?;
        self.resolve_query(QueryFamily::Id(family_id), weight)
    }

    fn resolve_generic(
        &mut self,
        family: GenericFamily,
        weight: f32,
    ) -> Option<ResolvedSystemFont> {
        self.resolve_query(QueryFamily::Generic(family), weight)
    }

    fn resolve_query(
        &mut self,
        family: QueryFamily<'_>,
        weight: f32,
    ) -> Option<ResolvedSystemFont> {
        let mut resolved = None;
        {
            let mut query = self.collection.query(&mut self.source_cache);
            query.set_families([family]);
            query.set_attributes(Attributes::new(
                FontWidth::NORMAL,
                FontStyle::Normal,
                FontWeight::new(weight),
            ));
            query.matches_with(|font| {
                resolved = Some((font.family.0, font.blob.as_ref().to_vec()));
                QueryStatus::Stop
            });
        }
        let (resolved_family, bytes) = resolved?;
        Some(ResolvedSystemFont {
            family: self.collection.family_name(resolved_family)?.to_owned(),
            bytes,
        })
    }
}

pub(super) fn register_system_fonts(
    runtime: &mut Runtime,
    requests: &[SystemFontRequest],
) -> Vec<RegisteredHostFont> {
    let mut catalog = SystemFontCatalog::new();
    requests
        .iter()
        .map(|request| {
            if let Some(resolved) = catalog.resolve(request.family, request.weight) {
                let id = runtime
                    .resources()
                    .register_font()
                    .expect("system font resource");
                runtime
                    .resources()
                    .complete_font(id, FontData::new(resolved.bytes))
                    .expect("system font data");
                RegisteredHostFont {
                    family: resolved.family,
                    #[cfg(test)]
                    resource_id: id.resource_id(),
                    #[cfg(test)]
                    system: true,
                }
            } else {
                let _id = runtime
                    .resources()
                    .builtin_font(request.fallback)
                    .expect("fallback font");
                RegisteredHostFont {
                    family: request.fallback.family().to_owned(),
                    #[cfg(test)]
                    resource_id: _id.resource_id(),
                    #[cfg(test)]
                    system: false,
                }
            }
        })
        .collect()
}

pub(super) fn register_system_ui_fonts(
    runtime: &mut Runtime,
    weights: &[f32],
    fallback: BuiltinFont,
) -> Vec<RegisteredHostFont> {
    let mut catalog = SystemFontCatalog::new();
    weights
        .iter()
        .map(|weight| {
            if let Some(resolved) = catalog.resolve_generic(GenericFamily::SystemUi, *weight) {
                let id = runtime
                    .resources()
                    .register_font()
                    .expect("system UI font resource");
                runtime
                    .resources()
                    .complete_font(id, FontData::new(resolved.bytes))
                    .expect("system UI font data");
                RegisteredHostFont {
                    family: resolved.family,
                    #[cfg(test)]
                    resource_id: id.resource_id(),
                    #[cfg(test)]
                    system: true,
                }
            } else {
                let _id = runtime
                    .resources()
                    .builtin_font(fallback)
                    .expect("system UI fallback font");
                RegisteredHostFont {
                    family: fallback.family().to_owned(),
                    #[cfg(test)]
                    resource_id: _id.resource_id(),
                    #[cfg(test)]
                    system: false,
                }
            }
        })
        .collect()
}

pub(super) fn font_descriptor(font: &RegisteredHostFont, points: f32, weight: f32) -> String {
    let logical_pixels = points * LOGICAL_PIXELS_PER_POINT;
    if weight == BOLD_FONT_WEIGHT {
        format!("bold {logical_pixels}px {}", font.family)
    } else if weight == NORMAL_FONT_WEIGHT {
        format!("{logical_pixels}px {}", font.family)
    } else {
        format!("{weight} {logical_pixels}px {}", font.family)
    }
}
