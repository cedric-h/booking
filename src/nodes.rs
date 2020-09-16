use std::time::{Instant, Duration};
use std::collections::HashMap;
use linked_hash_map::LinkedHashMap;

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
enum Story<SubStory> {
    Choices(ChoiceBundle<SubStory>),
    File(String),
    Fade(Fade<SubStory>),
    Title {
        text: String,
        size: f32,
    },
}
type StoryTree = Story<Box<IdentifiedStoryTree>>;
type SingleStory = Story<StoryId>;

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(from = "LinkedHashMap<String, SubStory>")]
struct ChoiceBundle<SubStory>(
    Vec<(String, SubStory)>,
    usize,
);

impl<SubStory> From<LinkedHashMap<String, SubStory>> for ChoiceBundle<SubStory> {
    fn from(m: LinkedHashMap<String, SubStory>) -> Self {
        Self(m.into_iter().collect(), 0)
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Fade<SubStory> {
    milliseconds: usize,
    #[serde(skip, default = "Instant::now")]
    start: Instant,
    node: SubStory,
    then: Option<SubStory>,
}
impl<SubStory> Fade<SubStory> {
    #[cfg(not(debug_assertions))]
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    #[cfg(debug_assertions)]
    fn elapsed(&self) -> Duration {
        self.start.elapsed() * 100
    }

    fn duration(&self) -> Duration {
        Duration::from_millis(self.milliseconds as u64)
    }

    fn complete(&self) -> bool {
        self.elapsed() > self.duration()
    }

    fn progress(&self) -> f32 {
        self.elapsed().as_secs_f32() / self.duration().as_secs_f32()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StoryId(u32);

#[derive(serde::Deserialize)]
#[serde(from = "StoryTree")]
struct IdentifiedStoryTree {
    story: StoryTree,
    id: StoryId,
}
impl From<StoryTree> for IdentifiedStoryTree {
    fn from(story: StoryTree) -> Self {
        IdentifiedStoryTree {
            story,
            id: StoryId(macroquad::rand::rand()),
        }
    }
}
type StoryMap = HashMap<StoryId, SingleStory>;
impl IdentifiedStoryTree {
    /// Turns a tree-based story structure into a linear hashmap of story nodes.
    fn flatten(&self) -> StoryMap {
        let mut sm = StoryMap::with_capacity(1000);
        self.add_single_story(&mut sm);
        sm
    }

    /// Returns the id of the topmost level story that was added, for convenience
    fn add_single_story(&self, sm: &mut StoryMap) -> StoryId {
        let single_story = match &self.story {
            &Story::Fade(Fade {
                milliseconds,
                start,
                ref node,
                ref then,
            }) => Story::Fade(Fade {
                milliseconds,
                start,
                node: node.add_single_story(sm),
                then: then.as_ref().map(|then| then.add_single_story(sm)),
            }),
            Story::Choices(ChoiceBundle(choices, selected)) => Story::Choices(ChoiceBundle(
                choices.iter().map(|(n, choice)| (n.clone(), choice.add_single_story(sm))).collect(),
                *selected,
            )),
            &Story::Title { ref text, size } => Story::Title {
                text: text.clone(),
                size,
            },
        };
        sm.insert(self.id, single_story);
        self.id
    }
}

struct Draw {
    transparency: u8,
    cursor_y: f32,
}
impl Draw {
    fn new() -> Self {
        Self {
            transparency: 255,
            cursor_y: 0.0,
        }
    }

    fn alpha(&self, macroquad::Color([r, g, b, _]): macroquad::Color) -> macroquad::Color {
        macroquad::Color([r, g, b, self.transparency])
    }

    fn draw_cursor_text(&mut self, text: &str, size: f32, x_offset: Option<f32>) {
        let xo = x_offset.unwrap_or(20.0);
        macroquad::draw_text(
            text,
            xo,
            self.cursor_y + 10.0,
            size,
            self.alpha(macroquad::WHITE)
        );
        self.cursor_y += size + 20.0;
    }

    fn render(&mut self, map: &StoryMap, story: &SingleStory) {
        use macroquad::*;
        use Story::*;

        match story {
            Choices(ChoiceBundle(choices, selected)) => {
                for (i, (name, _)) in choices.iter().enumerate() {
                    self.draw_cursor_text(name, 20.0, Some(40.0));
                    if i == *selected {
                        macroquad::draw_text(
                            ">",
                            20.0,
                            self.cursor_y - 30.0,
                            20.0,
                            self.alpha(macroquad::WHITE)
                        );
                    }
                }
            }
            Fade(fade) => {
                self.transparency = (fade.progress() * 255.0) as u8;
                self.render(map, &map[&fade.node]);
            }
            &Title { ref text, size } => self.draw_cursor_text(text, size, None),
        }
    }
}

pub struct Nodes {
    /// This field is not mutated,
    /// it's only used as a reference to jump to current story.
    story_map: StoryMap,
    /// The SingleStory here *is* actually mutated.
    current_story: Option<(StoryId, SingleStory)>,
    /// Stories left over :D
    scene: Vec<SingleStory>,
}
impl Nodes {
    pub fn new() -> Self {
        use crate::entry_yaml_bytes;

        #[derive(serde::Deserialize)]
        struct Config {
            story: IdentifiedStoryTree,
        }

        let Config { story } = serde_yaml::from_slice(&entry_yaml_bytes()).unwrap();
        let story_map = story.flatten();

        Nodes {
            scene: Vec::with_capacity(1000),
            current_story: Some((story.id, story_map[&story.id].clone())),
            story_map,
        }
    }

    /// Prepares a story to be all hip and current yo
    /// Seriously tho call this on a story before setting it to current_story
    fn freshen_story(&mut self, id: StoryId) -> (StoryId, SingleStory) {
        let mut story = self.story_map[&id].clone();
        if let Story::Fade(fade) = &mut story {
            fade.start = Instant::now();
        }
        (id, story)
    }

    pub fn update(&mut self) {
        use Story::*;
        self.current_story = self.current_story.take().and_then(|(id, mut story)| {
            match &mut story {
                Choices(ChoiceBundle(choices, selected)) => {
                    use macroquad::{is_key_pressed, KeyCode};
                    if is_key_pressed(KeyCode::Enter) {
                        let (_name, choice) = &choices[*selected];
                        return Some(self.freshen_story(*choice));
                    }
                    if is_key_pressed(KeyCode::Up) {
                        *selected = (*selected + 1) % choices.len();
                    }
                    if is_key_pressed(KeyCode::Down) {
                        let len = choices.len();
                        *selected = selected.checked_sub(1).unwrap_or(len - 1) % len;
                    }
                }
                Fade(fade) => {
                    if fade.complete() {
                        self.scene.push(self.story_map[&fade.node].clone());
                        return fade.then.map(|t| self.freshen_story(t))
                    }
                }
                Title { .. } => {},
            };

            Some((id, story))
        });
    }

    pub fn render(&mut self) {
        let mut draw = Draw::new();

        for s in &self.scene {
            draw.render(&self.story_map, s);
        }
        if let Some((_, cs)) = self.current_story.as_ref() {
            draw.render(&self.story_map, cs);
        }
    }
}
