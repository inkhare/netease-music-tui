use serde_json;
use serde_json::Value;
use serde::{Serialize, Deserialize};

use reqwest::Client;
use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, REFERER, USER_AGENT, HOST, ACCEPT_ENCODING, COOKIE};
use reqwest::StatusCode;

use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::string::{String, ToString};
use std::fmt::Debug;
use std::hash::Hash;
use std::borrow::Cow;
use std::time::Duration;

use super::model::user::{User, Profile, Login};
use super::model::artist::{TopArtistRes, Artist};
use super::model::song::{Song, Songs};
use super::model::album::{ArtistAlbums, Album, AlbumTrack, TopAlbumRes};
use super::model::search::{SearchTrackResult, SearchPlaylistResult, SearchPlaylists, SearchTracks, SearchArtistResult, SearchArtists, SearchAlbumResult, SearchAlbums};
use super::model::playlist::{PlaylistRes, Playlist, Track, PlaylistDetailRes, PlaylistDetail, PersonalFmRes, TopPlaylistRes};
use super::model::lyric::{LyricRes, Lyric};

use super::util::Encrypt;
use openssl::hash::{hash, MessageDigest};
use std::fs;
use chrono::prelude::*;

lazy_static! {
    /// HTTP Client
    pub static ref CLIENT: Client = Client::builder()
        .gzip(true)
        // .cookie_store(true)
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    RateLimited(Option<usize>),
    Other(u16)
}
impl failure::Fail for ApiError {}
impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Spotify API reported an error")
    }
}
impl From<&reqwest::Response> for ApiError {
    fn from(response: &reqwest::Response) -> Self {
        match response.status() {
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            StatusCode::TOO_MANY_REQUESTS => {
                if let Ok(duration) = response.headers()[reqwest::header::RETRY_AFTER].to_str() {
                    ApiError::RateLimited(duration.parse::<usize>().ok())
                }
                else {
                    ApiError::RateLimited(None)
                }
            },
            status => ApiError::Other(status.as_u16())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMusic {
    pub prefix: String
}

impl CloudMusic {

    pub fn default() -> CloudMusic {
        CloudMusic {
            prefix: "http://10.1.78.190:3000".to_owned(),
        }
    }

    ///send get request
    fn get(&self, url: &str, params: &mut HashMap<String, String>) -> Result<String, failure::Error> {
        if !params.is_empty() {
            let param: String = convert_map_to_string(params);
            let mut url_with_params = url.to_owned();
            url_with_params.push('?');
            url_with_params.push_str(&param);
            self.internal_call(Method::GET, &url_with_params, None)
        } else {
            self.internal_call(Method::GET, url, None)
        }
    }

    // send post request
    #[allow(unused)]
    fn post(&self, url: &str, params: &mut HashMap<String, String>) -> Result<String, failure::Error> {
        let mut csrf_token = String::new();
        params.insert("csrf_token".to_owned(), csrf_token);
        let params = Encrypt::encrypt_login(params);
        // let param = json!(params);
        let a = self.internal_call(Method::POST, &url, Some(params));
        Ok(a.unwrap())
    }

    fn internal_call(&self, method: Method, url: &str, payload: Option<String>) -> Result<String, failure::Error> {
        let mut url: Cow<str> = url.into();
        if !url.starts_with("http") {
            url = ["https://music.163.com", &url].concat().into();
        }

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/x-www-form-urlencoded".parse().unwrap());
        headers.insert(ACCEPT, "*/*".parse().unwrap());
        headers.insert(REFERER, "https://music.163.com".parse().unwrap());
        headers.insert(USER_AGENT, "User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:65.0) Gecko/20100101 Firefox/65.0".parse().unwrap());
        headers.insert(HOST, "music.163.com".parse().unwrap());
        headers.insert(ACCEPT_ENCODING, "gzip,deflate,br".parse().unwrap());

        match method {
            Method::POST => {
                let name = "os";
                let value = "pc";
                let local: DateTime<Local> = Local::now();
                let times = local.timestamp();
                let hextoken = hex::encode(hash(MessageDigest::md5(), &times.to_string().as_bytes()).unwrap());

                // read local save cookie
                let data = fs::read_to_string("cookie").unwrap_or(String::new());
                let make_cookie = format!("version=0;{}={};JSESSIONID-WYYY=%2FKSy%2B4xG6fYVld42G9E%2BxAj9OyjC0BYXENKxOIRH%5CR72cpy9aBjkohZ24BNkpjnBxlB6lzAG4D%5C%2FMNUZ7VUeRUeVPJKYu%2BKBnZJjEmqgpOx%2BU6VYmypKB%5CXb%2F3W7%2BDjOElCb8KlhDS2cRkxkTb9PBDXro41Oq7aBB6M6OStEK8E%2Flyc8%3A{}; _iuqxldmzr_=32; _ntes_nnid={},{}; _ntes_nuid={}; {}", name, value,times,hextoken,hextoken,times+50, data);
                headers.insert(COOKIE, make_cookie.parse().unwrap());

            }
            Method::GET => {
                let data = fs::read_to_string("cookie").unwrap_or(String::new());
                let make_cookie = format!("{}", data);
                headers.insert(COOKIE, make_cookie.parse().unwrap());
            }
            _ => {}
        }
        let mut response = {
            let builder = CLIENT
                .request(method, &url.into_owned())
                .headers(headers);

            // only add body if necessary
            // spotify rejects GET requests that have a body with a 400 response
            let builder = if let Some(data) = payload {
                builder.body(data)
            } else {
                builder
            };

            builder.send().unwrap()
        };
        let mut buf = String::new();

        self.store_cookies(&response);

        response
            .read_to_string(&mut buf)
            .expect("failed to read response");
        if response.status().is_success() {
            Ok(buf)
        } else if response.status() == 403 {
            Ok(buf)
        } else {
            Err(failure::Error::from(ApiError::from(&response)))
        }
    }

    fn store_cookies(&self, res: &reqwest::Response) {
        let cookies: Vec<String> = res.cookies()
            .into_iter()
            .map(|s| format!("{}={}", s.name().to_string(), s.value().to_string()))
            .collect();
        let mut c: String = cookies
            .into_iter()
            .map(|s| format!("{}; ", s))
            .collect();
        c.pop();
        if c.len() > 0 {
            let cookie_path = format!("cookie");
            fs::write(&cookie_path, &c).expect("Unable to write file");
        }
    }

    pub fn login(&self, email: &str, password: &str) -> Result<Profile, failure::Error> {
        let url = format!("/login");
        let mut params = HashMap::new();
        params.insert("email".to_owned(), email.to_string());
        params.insert("password".to_owned(), password.to_string());

        let result = self.get(&url, &mut params)?;
        let login = self.convert_result::<Login>(&result).unwrap();
        Ok(login.profile.unwrap())
    }

    pub fn phone_login(&self, phone: &str, password: &str) -> Result<Profile, failure::Error> {
        let password = hash(MessageDigest::md5(), password.as_bytes()).unwrap();
        let url = format!("/weapi/login/cellphone");
        let mut params = HashMap::new();
        params.insert("phone".to_owned(), phone.to_string());
        params.insert("password".to_owned(), hex::encode(password));
        params.insert("rememberLogin".to_owned(), "true".to_owned());

        let result = self.post(&url, &mut params)?;
        let login = self.convert_result::<Login>(&result).unwrap();
        Ok(login.profile.unwrap())
    }

    pub fn login_status(&self) -> Result<Option<Profile>, failure::Error> {
        let url = format!("/");
        match self.get(&url, &mut HashMap::new()) {
            Ok(r) => {
                let re = regex::Regex::new(
                    r#"userId:(?P<id>\d+),nickname:"(?P<nickname>\w+)""#,
                ).unwrap();
                if let Some(cap) = re.captures(&r) {
                    let uid = cap.name("id").unwrap().as_str().parse::<i32>().unwrap_or(0);
                    let nickname = cap.name("nickname").unwrap().as_str().to_owned();
                    Ok(Some(
                        Profile{
                            userId: Some(uid),
                            nickname: Some(nickname),
                            gender: None,
                            follows: None,
                            followeds: None,
                        }
                    ))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                panic!("api error {}", e);
            }
        }
    }

    pub fn user(&self, user_id: &str) -> Result<User, failure::Error> {
        let url = format!("/user/detail");
        // url.push_str(&trid);
        let mut params = HashMap::new();
        params.insert("uid".to_owned(), user_id.to_string());

        let result = self.get(&url, &mut params)?;
        self.convert_result::<User>(&result)
    }

    // get song url
    pub fn get_song_url(&self, song_id: &str) -> Result<Song, failure::Error> {
        let url = format!("/weapi/song/enhance/player/url");
        let mut params = HashMap::new();
        let song_id = song_id.to_string().parse::<u32>().unwrap();
        params.insert("ids".to_owned(), serde_json::to_string(&vec![song_id]).unwrap_or("[]".to_owned()));
        params.insert("br".to_owned(), 999000.to_string());

        // send request
        let result = self.post(&url, &mut params)?;
        let songs = self.convert_result::<Songs>(&result).unwrap();
        Ok(songs.data[0].clone())
    }

    // user playlist api
    pub fn user_playlists(&self, uid: &str) -> Result<Vec<Playlist>, failure::Error> {
        let url = format!("/weapi/user/playlist");
        let mut params = HashMap::new();
        params.insert("uid".to_owned(), uid.to_string());
        params.insert("limit".to_owned(), 1000.to_string());
        params.insert("offest".to_owned(), 0.to_string());
        params.insert("csrf_token".to_owned(), "".to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<PlaylistRes>(&result).unwrap();
        Ok(res.playlist.clone())
    }

    // get playlist detail api
    pub fn playlist_detail(&self, playlist_id: &str) -> Result<PlaylistDetail, failure::Error> {
        let url = format!("/weapi/v3/playlist/detail");
        let mut params = HashMap::new();
        params.insert("id".to_owned(), playlist_id.to_string());
        params.insert("total".to_owned(), true.to_string());
        params.insert("limit".to_owned(), 1000.to_string());
        params.insert("offest".to_owned(), 0.to_string());
        params.insert("n".to_owned(), 1000.to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<PlaylistDetailRes>(&result).unwrap();
        Ok(res.playlist.unwrap().clone())
    }

    // artist albums
    pub fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>, failure::Error> {
        let url = format!("/weapi/artist/albums/{}", artist_id);
        let mut params = HashMap::new();
        params.insert("total".to_owned(), true.to_string());
        params.insert("limit".to_owned(), 1000.to_string());
        params.insert("offest".to_owned(), 0.to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<ArtistAlbums>(&result).unwrap();
        Ok(res.hotAlbums.unwrap().clone())
    }

    // album track
    pub fn album_track(&self, album_id: &str) -> Result<AlbumTrack, failure::Error> {
        let url = format!("/weapi/v1/album/{}", album_id);
        let mut params = HashMap::new();

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<AlbumTrack>(&result).unwrap();
        Ok(res)
    }

    // lyric
    pub fn lyric(&self, track_id: &str) -> Result<Vec<Lyric>, failure::Error> {
        let url = format!("/weapi/song/lyric");
        let mut params = HashMap::new();
        params.insert("id".to_owned(), track_id.to_string());
        params.insert("os".to_owned(), "osx".to_owned());
        params.insert("lv".to_owned(), "-1".to_owned());
        params.insert("kv".to_owned(), "-1".to_owned());
        params.insert("tv".to_owned(), "-1".to_owned());

        let result = self.post(&url, &mut params)?;
        match self.convert_result::<LyricRes>(&result) {
            Ok(res) => {
                let lyric: Vec<Lyric> = res.lrc.lyric
                    .lines()
                    .map(|s| {
                        let re = regex::Regex::new(
                            r#"\[(\w+):(\w+)\.(\w+)\](.*?)$"#,
                        ).unwrap();
                        if let Some(cap) = re.captures(&s) {
                            let minite = cap[1].parse::<u64>().unwrap_or(0);
                            let second = cap[2].parse::<u64>().unwrap_or(0);
                            let nano = cap[3][..1].parse::<u32>().unwrap_or(0) * 10000000;
                            let lyric_value = cap[4].to_string();
                            let duration_min = minite * 60 + second;
                            Lyric {
                                value: lyric_value,
                                timeline: Duration::new(duration_min, nano),
                            }
                        } else {
                            Lyric {
                                value: String::new(),
                                timeline: Duration::new(0, 0),
                            }
                        }
                    })
                    .collect();
                Ok(lyric)
            }
            Err(_) => {
                let lyric = vec![
                    Lyric {
                        value: "no lyric".to_string(),
                        timeline: Duration::new(0, 0),
                    }
                ];
                Ok(lyric)
            }
        }
    }

    // search api
    pub fn search(&self, keyword: &str, search_type: &str, limit: i32, offset: i32) -> Result<String, failure::Error> {
        let url = format!("/weapi/search/get");
        let mut params = HashMap::new();
        params.insert("s".to_owned(), keyword.to_string());
        params.insert("type".to_owned(), search_type.to_string());
        params.insert("limit".to_owned(), limit.to_string());
        params.insert("offset".to_owned(), offset.to_string());

        // send request
        self.post(&url, &mut params)
    }

    // search for track
    pub fn search_track(&self, keyword: &str, limit: i32, offset: i32) -> Result<SearchTracks, failure::Error> {
        let result = self.search(keyword, "1", limit, offset)?;
        let res = self.convert_result::<SearchTrackResult>(&result)?;
        Ok(res.result)
    }

    // search for playlist
    pub fn search_playlist(&self, keyword: &str, limit: i32, offset: i32) -> Result<SearchPlaylists, failure::Error> {
        let result = self.search(keyword, "1000", limit, offset)?;
        let res = self.convert_result::<SearchPlaylistResult>(&result)?;
        Ok(res.result)
    }

    // search for artist
    pub fn search_artist(&self, keyword: &str, limit: i32, offset: i32) -> Result<SearchArtists, failure::Error> {
        let result = self.search(keyword, "100", limit, offset)?;
        let res = self.convert_result::<SearchArtistResult>(&result)?;
        Ok(res.result)
    }

    // search for album
    pub fn search_album(&self, keyword: &str, limit: i32, offset: i32) -> Result<SearchAlbums, failure::Error> {
        let result = self.search(keyword, "10", limit, offset)?;
        let res = self.convert_result::<SearchAlbumResult>(&result)?;
        Ok(res.result)
    }

    // get user personal fm
    pub fn personal_fm(&self) -> Result<Vec<Track>, failure::Error> {
        let url = format!("/weapi/v1/radio/get");
        let mut params = HashMap::new();

        // send request
        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<PersonalFmRes>(&result).unwrap();
        Ok(res.data)
    }

    // top songlist
    pub fn top_playlists(&self) -> Result<Vec<Playlist>, failure::Error> {
        let url = format!("/weapi/playlist/list");
        let mut params = HashMap::new();
        params.insert("cat".to_owned(), "全部".to_string());
        params.insert("order".to_owned(), "hot".to_string());
        params.insert("limit".to_owned(), 50.to_string());
        params.insert("offset".to_owned(), 0.to_string());
        params.insert("total".to_owned(), true.to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<TopPlaylistRes>(&result).unwrap();
        Ok(res.playlists)
    }

    // top artist
    pub fn top_artists(&self) -> Result<Vec<Artist>, failure::Error> {
        let url = format!("/weapi/artist/top");
        let mut params = HashMap::new();
        params.insert("limit".to_owned(), 50.to_string());
        params.insert("offset".to_owned(), 0.to_string());
        params.insert("total".to_owned(), true.to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<TopArtistRes>(&result).unwrap();
        Ok(res.artists)
    }

    // top album
    pub fn top_albums(&self) -> Result<Vec<Album>, failure::Error> {
        let url = format!("/weapi/album/new");
        let mut params = HashMap::new();
        params.insert("area".to_owned(), "ALL".to_string());
        params.insert("limit".to_owned(), 50.to_string());
        params.insert("offset".to_owned(), 0.to_string());
        params.insert("total".to_owned(), true.to_string());

        let result = self.post(&url, &mut params)?;
        let res = self.convert_result::<TopAlbumRes>(&result).unwrap();
        Ok(res.albums)
    }

    // like track
    pub fn like(&self, track_id: &str, like: bool) -> Result<String, failure::Error> {
        let url = format!("/weapi/radio/like");
        let mut params = HashMap::new();
        params.insert("alg".to_owned(), "itembased".to_owned());
        params.insert("trackId".to_owned(), track_id.to_string());
        params.insert("like".to_owned(), like.to_string());
        params.insert("time".to_owned(), "25".to_owned());

        let result = self.post(&url, &mut params)?;
        Ok("ok".to_string())
    }

    pub fn convert_result<'a, T: Deserialize<'a>>(&self, input: &'a str) -> Result<T, failure::Error> {
        let result = serde_json::from_str::<T>(input)
            .map_err(|e| format_err!("convert result failed, reason: {:?}; content: [{:?}]", e,input))?;
        Ok(result)
    }

    pub fn login_v1(&self, email: &str, password: &str) -> Result<String, failure::Error> {
        let url = format!("/weapi/login");
        let client_token =
            "1_jVUMqWEPke0/1/Vu56xCmJpo5vP1grjn_SOVVDzOc78w8OKLVZ2JH7IfkjSXqgfmh";
        let mut params = HashMap::new();
        params.insert("clientToken".to_owned(), client_token.to_string());
        params.insert("username".to_owned(), email.to_string());
        params.insert("password".to_owned(), hex::encode(password.to_string()));
        params.insert("rememberLogin".to_owned(), "true".to_owned());

        let result = self.post(&url, &mut params)?;
        // let login = self.convert_result::<Login>(&result).unwrap();
        // Ok(login.profile.unwrap())
        Ok("ddd".to_string())
    }
}

pub fn convert_map_to_string<K: Debug + Eq + Hash+ ToString,
V: Debug+ToString>(map: &HashMap<K, V>) -> String{
    let mut string: String = String::new();
    for (key, value) in map.iter() {
        string.push_str(&key.to_string());
        string.push_str("=");
        string.push_str(&value.to_string());
        string.push_str("&");
    }
    string
}
