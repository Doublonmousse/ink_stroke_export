extern crate nalgebra as na;
use anyhow::{bail, Ok};
use rnote_compose::penpath::Element;
use rnote_compose::shapes::Rectangle;
use rnote_compose::style::smooth;
use rnote_compose::style::smooth::SmoothOptions;
use rnote_compose::Color;
use rnote_compose::PenPath;
use rnote_engine::document::background::PatternStyle;
use rnote_engine::document::Layout;
use rnote_engine::store::chrono_comp::StrokeLayer;
use rnote_engine::strokes::BrushStroke;
use rnote_engine::strokes::Stroke;
use rnote_engine::Engine;
use serde::{de::Error, Deserialize, Deserializer};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::current;

fn main() -> anyhow::Result<()> {
    smol::block_on(async { parse_pages().await })
}

async fn get_root_folder() -> anyhow::Result<PathBuf> {
    let path_str = std::env::args().nth(1).expect("no pattern given");
    let root_folder = PathBuf::from(path_str);
    if !(root_folder.exists() && root_folder.is_dir()) {
        bail!(anyhow::anyhow!(
            "could not find the path provided or is not a folder"
        ));
    } else {
        Ok(root_folder)
    }
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct Metadata {
    /// Title if it exists
    /// There can be pages with no name
    pageTitle: Option<String>,
    /// background Pattern
    backgroundPattern: String,
}

#[derive(Deserialize, Debug)]
struct BoundingBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
struct Items {
    /// Gives the type of the element
    #[serde(rename(deserialize = "type"))]
    Type: String,
    /// specific to stroke
    X: Option<Vec<f64>>,
    /// specific to stroke
    Y: Option<Vec<f64>>,
    /// specific to stroke
    F: Option<Vec<f64>>,
    // text would be in label and you'd then have a bounding box PER glyph
    // For now ignoring (to not have to deal with reconstructing text from individual
    // glyphs)
    // specific to glyph
    // label: Option<String>,
    /// specific to line
    x1: Option<f64>,
    /// specific to line
    x2: Option<f64>,
    /// specific to line
    y1: Option<f64>,
    /// specific to line
    y2: Option<f64>,
}

#[derive(Deserialize, Debug, Copy, Clone)]
struct Spans {
    #[serde(rename(deserialize = "last-item"))]
    last_item: usize,
    #[serde(deserialize_with = "parse_style")]
    style: Style,
}

#[derive(Default, Debug, Copy, Clone)]
struct Style {
    pen_width: f64,
    color: (u8, u8, u8, u8),
}

fn parse_style<'de, D>(deserializer: D) -> Result<Style, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    // split by ;
    let mut style = Style::default();
    for element in s.split(";") {
        // split value/element
        let mut split_val_el = element.split(":").into_iter();

        let name = split_val_el
            .next()
            .ok_or_else(|| D::Error::custom(String::from("could not parse style")))?
            .trim();
        let value = split_val_el
            .next()
            .ok_or_else(|| D::Error::custom(String::from("could not parse style")))?
            .trim();

        //println!("name and value : {:?}  {:?}", name, value);
        match name {
            "-myscript-pen-width" => {
                style.pen_width = value
                    .parse::<f64>()
                    .map_err(|_| D::Error::custom(String::from("could not parse pen width")))?;
            }
            "color" => {
                // get the color from the hex
                style.color = get_color_from_hex(value.to_owned())
                    .map_err(|_| D::Error::custom(String::from("could not parse color")))?;
            }
            _ => {
                //ignore for now
            }
        }
    }
    std::result::Result::Ok(style)
}

fn wrapped_parse_style<'de, D>(deserializer: D) -> Result<Option<Style>, D::Error>
where
    D: Deserializer<'de>,
{
    let result = parse_style(deserializer);
    match result {
        std::result::Result::Ok(style) => std::result::Result::Ok(Some(style)),
        Err(e) => Err(e),
    }
}

fn get_color_from_hex(s: String) -> Result<(u8, u8, u8, u8), ()> {
    if s.len() == 9 {
        let r = u8::from_str_radix(&s[1..=2], 16).map_err(|_| ())?;
        let g = u8::from_str_radix(&s[3..=4], 16).map_err(|_| ())?;
        let b = u8::from_str_radix(&s[5..=6], 16).map_err(|_| ())?;
        let a = u8::from_str_radix(&s[7..=8], 16).map_err(|_| ())?;
        return std::result::Result::Ok((r, g, b, a));
    } else {
        Err(())
    }
}

#[derive(Deserialize, Debug, Clone)]
struct NeboElement {
    #[serde(rename(deserialize = "type"))]
    type_str: String,
    #[serde(rename(deserialize = "url"))]
    url: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    items: Option<Vec<Items>>,
    spans: Option<Vec<Spans>>,
    #[serde(default, deserialize_with = "wrapped_parse_style")]
    style: Option<Style>,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
/// https://developer.myscript.com/docs/interactive-ink/2.0/reference/jiix/ for the reference on the
/// file format used here
struct StrokeData {
    elements: Vec<NeboElement>,
}

async fn parse_pages() -> anyhow::Result<()> {
    let root_folder = get_root_folder().await?;
    if !(root_folder.exists() && root_folder.is_dir()) {
        bail!(anyhow::anyhow!(
            "could not find the path provided or is not a folder"
        ));
    } else {
        println!("{:?}", root_folder.canonicalize()?);
        // we expect a list of folders
        for folder in root_folder
            .read_dir()
            .expect("expected dir")
            .filter(|x| x.is_ok() && x.as_ref().unwrap().metadata().unwrap().is_dir())
            .map(|x| x.unwrap())
        {
            let folder_name = folder
                .file_name()
                .to_owned()
                .into_string()
                .unwrap()
                .replace(".nebo", "");
            println!("collection : {:?}", folder_name);
            // folder with the structure
            // -- objects (images with id)
            // -- pages
            //  |-- pageids
            //       |-- pageid.jiix (stroke data)
            //       |-- meta.json (bakground and name of page)

            let mut unnamed_counter: usize = 0;
            // we iterate over the pages
            for page in folder
                .path()
                .join("pages/")
                .read_dir()
                .expect("expected dir")
                .filter(|x| x.is_ok() && x.as_ref().unwrap().metadata().unwrap().is_dir())
                .map(|x| x.unwrap())
            {
                // parse the meta.json file
                let metadata_path = page.path().join("meta.json");
                let metadata_str = std::fs::read_to_string(&metadata_path)?;
                let mut metadata: Metadata = serde_json::from_str(&metadata_str).map_err(|_| {
                    anyhow::anyhow!("couldn't parse metadata at path {:?}", &metadata_path)
                })?;
                if metadata.pageTitle.is_none() {
                    unnamed_counter += 1;
                    // give a default name for unnammed pages
                    metadata.pageTitle = Some(format!("unnamed_{unnamed_counter}"));
                    //println!("counter increased by one {:?}", unnamed_counter);
                }
                println!("metadata : {:?}, page id {:?}", metadata, page.path());

                // parse the jiix
                let jiix_path = page
                    .path()
                    .join(format!("{}.jiix", page.file_name().to_str().unwrap()));
                let jiix_str = std::fs::read_to_string(&jiix_path).map_err(|_| {
                    anyhow::anyhow!(format!(
                        "could not parse the .jiix file at path {:?}",
                        &jiix_path
                    ))
                })?;
                let stroke_data: StrokeData = serde_json::from_str(&jiix_str).map_err(|_| {
                    anyhow::anyhow!("couldn't parse .jiix file at path at path {:?}", jiix_path)
                })?;
                // we have everything to call a helper function
                // with
                // - paths (to find images)
                // - name of folder
                // - name of page
                // - stroke data
                // - metadata
                // And add strokes one by one + images to a rnote file
                create_rnote_file(&folder.path(), stroke_data, metadata, &folder_name).await?;
            }
        }
    }
    Ok(())
}

/// create a .rnote file from the parsed nebo data
///
/// `page_root` : root of the page folder (meta.json and pageid.jiix)
/// `collection_root` : root of the collection folder (objects and pages folders)
async fn create_rnote_file(
    collection_root: &PathBuf,
    stroke_data: StrokeData,
    metadata: Metadata,
    folder_name: &String,
) -> anyhow::Result<()> {
    // create the end filename
    let mut end_filename = folder_name.to_owned();
    end_filename.push('_');
    end_filename.push_str(&metadata.pageTitle.unwrap());
    end_filename.push_str(".rnote");

    let mut engine = Engine::default();
    engine.document.layout = Layout::Infinite;

    // set the background to white
    engine.document.background.color = Color::WHITE;
    // do not show borders
    engine.document.format.show_borders = false;

    // iterate over the stroke data
    if metadata.backgroundPattern == "grid" {
        engine.document.background.pattern = PatternStyle::Grid;
        engine.document.background.pattern_size = na::Vector2::new(32.0, 32.0);
    }
    
    let mut stroke_collect: Vec<(Stroke,Option<StrokeLayer>)> = vec![];

    for element in stroke_data.elements {
        let mut span_iterator = element.spans.unwrap_or(vec![]).into_iter();
        let mut current_span = span_iterator.next();
        match element.type_str.as_str() {
            "Raw Content" | "Edge" | "Node" => {
                for (element_index, item) in element.items.unwrap_or(vec![]).into_iter().enumerate()
                {
                    match item.Type.as_str() {
                        "stroke" => {
                            let mut smooth_options = SmoothOptions::default();

                            if current_span.is_some() {
                                if !(element_index <= current_span.unwrap().last_item) {
                                    current_span = span_iterator.next();
                                }

                                // verify this is still not none
                                if current_span.is_some() {
                                    let (r, g, b, a) = current_span.unwrap().style.color;
                                    if a < 10 {
                                        println!("transparent color !!! {:?}", current_span.unwrap().style.color);
                                    }
                                    smooth_options.stroke_color = Some(Color::new(
                                        r as f64 / 255.0,
                                        g as f64 / 255.0,
                                        b as f64 / 255.0,
                                        1.0//a as f64 / 255.0,
                                    ));
                                    smooth_options.stroke_width =
                                        current_span.unwrap().style.pen_width * 7.0;
                                    //to verify
                                }
                            }

                            // need to get stroke width and color from the spans of the element iteslf
                            let penpath = PenPath::try_from_elements(
                                item.X
                                    .unwrap()
                                    .into_iter()
                                    .zip(item.Y.unwrap())
                                    .zip(item.F.unwrap())
                                    .map(|((x, y), f)| {
                                        Element::new(na::vector![7.0 * x, 7.0 * y], f)
                                        // empirically inferred multiplicative value to have the grid match
                                        // between the two files. Not perfect but pretty close
                                    }),
                            )
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Could not generate pen path from coordinates vector"
                                )
                            })?;

                            let new_stroke = BrushStroke::from_penpath(
                                penpath,
                                rnote_compose::Style::Smooth(smooth_options),
                            );
                            let layer = StrokeLayer::UserLayer(0);
                            stroke_collect.push(
                                (Stroke::BrushStroke(new_stroke), Some(layer))
                            );

                        }
                        "glyph" => {
                            // for now we ignore it
                            // we don't want to recompose text from individual letters ...
                        }
                        "arc" => {
                            // we ignore arcs for now
                        }
                        "line" => {
                            // we will still use a penpath here
                            let mut smooth_options = SmoothOptions::default();
                            smooth_options.stroke_width = element.style.unwrap().pen_width;
                            let (r, g, b, a) = element.style.unwrap().color;
                            smooth_options.stroke_color = Some(Color::new(
                                r as f64 / 255.0,
                                g as f64 / 255.0,
                                b as f64 / 255.0,
                                a as f64 / 255.0,
                            ));
                            let penpath = PenPath::try_from_elements(vec![
                                Element::new(
                                    7.0 * na::Vector2::new(item.x1.unwrap(), item.y1.unwrap()),
                                    1.0,
                                ),
                                Element::new(
                                    7.0 * na::Vector2::new(item.x2.unwrap(), item.y2.unwrap()),
                                    1.0,
                                ),
                            ])
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Could not generate pen path from coordinates vector"
                                )
                            })?;
                            let new_stroke = BrushStroke::from_penpath(
                                penpath,
                                rnote_compose::Style::Smooth(smooth_options),
                            );
                            let layer = StrokeLayer::UserLayer(0);
                            stroke_collect.push(
                                (Stroke::BrushStroke(new_stroke), Some(layer))
                            );
                            // let _ = engine
                            //     .import_generated_content(vec![
                            //         (Stroke::BrushStroke(new_stroke), Some(layer))
                            //     ], false);
                        }
                        _ => {
                            bail!(anyhow::anyhow!("unexpected input"));
                        }
                    }
                }
            }
            "Image" => {
                let file_name = element.url.as_ref().unwrap();
                // get the image
                let file_path = collection_root.join(format!("objects/{file_name}"));
                if file_path.exists() {
                    let bytes = std::fs::read(file_path)?;
                    let mut bitmapimage = engine
                        .generate_bitmapimage_from_bytes(
                            na::Vector2::new(element.x.unwrap() * 7.0, element.y.unwrap() * 7.0),
                            bytes,
                            false,
                        )
                        .await??;

                    // modify the size
                    bitmapimage.rectangle = Rectangle::from_corners(
                        7.0 * na::Vector2::new(element.x.unwrap(), element.y.unwrap()),
                        7.0 * na::Vector2::new(
                            element.x.unwrap() + element.width.unwrap(),
                            element.y.unwrap() + element.height.unwrap(),
                        ),
                        
                    );
                    stroke_collect.push(
                        (Stroke::BitmapImage(bitmapimage), None)
                    );

                } else {
                    bail!(anyhow::anyhow!(
                        "could not find the image at path {:?}",
                        file_path
                    ));
                }
            }
            _ => {
                println!("unexpected input, ignoring");
            }
        }
    }
    // push all strokes
    let _ = engine
    .import_generated_content(stroke_collect, false);

    // we finished to push all strokes
    let rnote_bytes = engine.save_as_rnote_bytes(end_filename.clone()).await??;
    let mut fh = File::create(Path::new(&end_filename))?;
    fh.write_all(&rnote_bytes)?;
    fh.sync_all()?;

    Ok(())
}
