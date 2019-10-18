extern crate gstreamer as gst;
extern crate gstreamer_player as gst_player;
use tui::layout::{Layout, Constraint, Direction, Rect};
use super::model::playlist::{PlaylistDetail, Track};
use super::api::CloudMusic;

use gst::prelude::*;

const DEFAULT_ROUTE: Route = Route {
    id: RouteId::Home,
    active_block: ActiveBlock::Empty,
    hovered_block: ActiveBlock::Library,
};

pub struct Route {
    pub id: RouteId,
    pub active_block: ActiveBlock,
    pub hovered_block: ActiveBlock,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
    AlbumTracks,
    AlbumList,
    Artist,
    Error,
    Home,
    RecentlyPlayed,
    Search,
    SelectedDevice,
    TrackTable,
    MadeForYou,
    Artists,
    Podcasts,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
    AlbumTracks,
    AlbumList,
    Artist,
    Empty,
    Error,
    HelpMenu,
    Home,
    Input,
    Library,
    MyPlaylists,
    Podcasts,
    RecentlyPlayed,
    SearchResultBlock,
    SelectDevice,
    TrackTable,
    MadeForYou,
    Artists,
}

#[derive(Default)]
pub struct TrackTable {
    pub tracks: Vec<Track>,
    pub selected_index: usize,
}

pub struct App {
    navigation_stack: Vec<Route>,
    pub player: gst_player::Player,
    pub size: Rect,
    pub input: String,
    pub song_progress_ms: u128,
    pub playlist: Option<PlaylistDetail>,
    pub selected_playlist_index: Option<usize>,
    pub track_table: TrackTable,
    pub cloud_music: CloudMusic,
}

impl App {
    pub fn new() -> App {

        let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
        let music_player = gst_player::Player::new(
            None,
            Some(&dispatcher.upcast::<gst_player::PlayerSignalDispatcher>()),
        );

        App {
            navigation_stack: vec![DEFAULT_ROUTE],
            player: music_player,
            size: Rect::default(),
            input: String::new(),
            song_progress_ms: 0,
            playlist: None,
            selected_playlist_index: None,
            track_table: Default::default(),
            cloud_music: CloudMusic::default(),
        }
    }

    pub fn increase_volume(&mut self) {
        let current = self.player.get_volume();
        let volume = current + 0.1_f64;
        self.player.set_volume(volume)
    }

    pub fn decrease_volume(&mut self) {
        let current = self.player.get_volume();
        let volume = current - 0.1_f64;
        self.player.set_volume(volume)
    }

    pub fn get_current_route(&self) -> &Route {
        match self.navigation_stack.last() {
            Some(route) => route,
            None => &DEFAULT_ROUTE, // if for some reason there is no route return the default
        }
    }

    pub fn push_navigation_stack(
        &mut self,
        next_route_id: RouteId,
        next_active_block: ActiveBlock,
    ) {
        self.navigation_stack.push(Route {
            id: next_route_id,
            active_block: next_active_block,
            hovered_block: next_active_block,
        });
    }

    pub fn get_playlist_tracks(&mut self, playlist_id: String) {
        self.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable)
    }

    pub fn start_playback(
        &mut self,
        id: String,
    ) {
        let song = self.cloud_music.song(&id).unwrap();
        let url = song.url.unwrap().to_string();
        self.player.set_uri(&url);
        self.player.play();
    }
}
