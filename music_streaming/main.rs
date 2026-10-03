//! Music streaming service. Full requirements: see `readme.md`. Single-file,
//! same architectural DNA as movie_ticket_booking/flight_management (guarded
//! RwLock maps, thiserror, indexed search, Arc composition root) scaled down
//! to this domain's actual shape: read-heavy catalog + per-user session
//! state, no seat/payment contention to guard against.

// # Designing an Online Music Streaming Service Like Spotify
//
// ## Requirements
// 1. The music streaming service should allow users to browse and search for songs, albums, and artists.
// 2. Users should be able to create and manage playlists.
// 3. The system should support user authentication and authorization.
// 4. Users should be able to play, pause, skip, and seek within songs.
// 5. The system should recommend songs and playlists based on user preferences and listening history.
// 6. The system should handle concurrent requests and ensure smooth streaming experience for multiple users.
// 7. The system should be scalable and handle a large volume of songs and users.
// 8. The system should be extensible to support additional features such as social sharing and offline playback.
//
// ## Classes, Interfaces and Enumerations
// 1. The **Song**, **Album**, and **Artist** classes represent the basic entities in the music streaming
// service, with properties such as ID, title, artist, album, duration, and relationships between them.
// 2. The **User** class represents a user of the music streaming service, with properties like ID, username,
// password, and a list of playlists.
// 3. The **Playlist** class represents a user-created playlist, containing a list of songs.
// 4. The **MusicLibrary** class serves as a central repository for storing and managing songs, albums, and
// artists. It follows the Singleton pattern to ensure a single instance of the music library.
// 5. The **UserManager** class handles user registration, login, and other user-related operations. It also
// follows the Singleton pattern.
// 6. The **MusicPlayer** class represents the music playback functionality, allowing users to play, pause,
// skip, and seek within songs.
// 7. The **MusicRecommender** class generates song recommendations based on user preferences and listening
// history. It follows the Singleton pattern.
// 8. The **MusicStreamingService** class is the main entry point of the music streaming service. It initializes
// the necessary components, handles user requests, and manages the overall functionality of the service.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;
use uuid::Uuid;

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StreamingError {
    #[error("username '{0}' already taken")]
    UsernameTaken(String),
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("user not found")]
    UserNotFound,
    #[error("song '{0}' not found")]
    SongNotFound(Uuid),
    #[error("playlist not found")]
    PlaylistNotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("no song is currently loaded in the player")]
    NothingPlaying,
    #[error("seek position {0}s exceeds song duration {1}s")]
    SeekOutOfBounds(u32, u32),
}

pub type StreamingResult<T> = std::result::Result<T, StreamingError>;

// ---------- Catalog: Artist, Album, Song ----------

#[derive(Debug, Clone)]
pub struct Artist {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub id: Uuid,
    pub title: String,
    pub artist_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub id: Uuid,
    pub title: String,
    pub artist_id: Uuid,
    pub album_id: Uuid,
    pub duration_secs: u32,
    pub genre: String,
}

/// Central catalog. Read-mostly, so plain RwLock<HashMap> is enough; a
/// dedicated search index (see MusicLibrary::search) avoids scanning
/// everything on every query without adding a second store to keep in sync.
pub struct MusicLibrary {
    artists: RwLock<HashMap<Uuid, Artist>>,
    albums: RwLock<HashMap<Uuid, Album>>,
    songs: RwLock<HashMap<Uuid, Song>>,
    /// lowercase word -> song ids whose title/genre contain that word.
    search_index: RwLock<HashMap<String, Vec<Uuid>>>,
}

impl MusicLibrary {
    pub fn new() -> Self {
        Self {
            artists: RwLock::new(HashMap::new()),
            albums: RwLock::new(HashMap::new()),
            songs: RwLock::new(HashMap::new()),
            search_index: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_artist(&self, name: impl Into<String>) -> Artist {
        let artist = Artist {
            id: Uuid::new_v4(),
            name: name.into(),
        };
        write_guard(&self.artists).insert(artist.id, artist.clone());
        artist
    }

    pub fn add_album(&self, title: impl Into<String>, artist_id: Uuid) -> Album {
        let album = Album {
            id: Uuid::new_v4(),
            title: title.into(),
            artist_id,
        };
        write_guard(&self.albums).insert(album.id, album.clone());
        album
    }

    pub fn add_song(
        &self,
        title: impl Into<String>,
        artist_id: Uuid,
        album_id: Uuid,
        duration_secs: u32,
        genre: impl Into<String>,
    ) -> Song {
        let song = Song {
            id: Uuid::new_v4(),
            title: title.into(),
            artist_id,
            album_id,
            duration_secs,
            genre: genre.into(),
        };
        write_guard(&self.songs).insert(song.id, song.clone());
        let mut index = write_guard(&self.search_index);
        for word in index_words(&song.title, &song.genre) {
            index.entry(word).or_default().push(song.id);
        }
        song
    }

    pub fn song(&self, id: Uuid) -> StreamingResult<Song> {
        read_guard(&self.songs)
            .get(&id)
            .cloned()
            .ok_or(StreamingError::SongNotFound(id))
    }

    pub fn artist(&self, id: Uuid) -> Option<Artist> {
        read_guard(&self.artists).get(&id).cloned()
    }

    pub fn album(&self, id: Uuid) -> Option<Album> {
        read_guard(&self.albums).get(&id).cloned()
    }

    pub fn songs_in_album(&self, album_id: Uuid) -> Vec<Song> {
        read_guard(&self.songs)
            .values()
            .filter(|s| s.album_id == album_id)
            .cloned()
            .collect()
    }

    /// Search songs by (partial, case-insensitive) title or genre word.
    pub fn search(&self, query: &str) -> Vec<Song> {
        let word = query.trim().to_lowercase();
        if word.is_empty() {
            return Vec::new();
        }
        let ids: Vec<Uuid> = read_guard(&self.search_index)
            .iter()
            .filter(|(indexed, _)| indexed.contains(&word))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        let songs = read_guard(&self.songs);
        let mut seen = std::collections::HashSet::new();
        ids.into_iter()
            .filter(|id| seen.insert(*id))
            .filter_map(|id| songs.get(&id).cloned())
            .collect()
    }
}

impl Default for MusicLibrary {
    fn default() -> Self {
        Self::new()
    }
}

fn index_words(title: &str, genre: &str) -> Vec<String> {
    let mut words: Vec<String> = title
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    words.push(genre.to_lowercase());
    words
}

// ---------- Users & auth ----------

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    password_hash: u64,
}

/// Not a real password hash - a placeholder so credentials are never
/// stored in plaintext. Swap for argon2/bcrypt before this touches prod.
fn hash_password(password: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    password.hash(&mut hasher);
    hasher.finish()
}

pub struct UserManager {
    users: RwLock<HashMap<Uuid, User>>,
    by_username: RwLock<HashMap<String, Uuid>>,
    /// listening history per user, most recent last - feeds the recommender.
    history: RwLock<HashMap<Uuid, Vec<Uuid>>>,
}

impl UserManager {
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            by_username: RwLock::new(HashMap::new()),
            history: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, username: &str, password: &str) -> StreamingResult<User> {
        let mut by_username = write_guard(&self.by_username);
        if by_username.contains_key(username) {
            return Err(StreamingError::UsernameTaken(username.to_string()));
        }
        let user = User {
            id: Uuid::new_v4(),
            username: username.to_string(),
            password_hash: hash_password(password),
        };
        by_username.insert(username.to_string(), user.id);
        write_guard(&self.users).insert(user.id, user.clone());
        Ok(user)
    }

    pub fn login(&self, username: &str, password: &str) -> StreamingResult<User> {
        let id = *read_guard(&self.by_username)
            .get(username)
            .ok_or(StreamingError::InvalidCredentials)?;
        let user = read_guard(&self.users)
            .get(&id)
            .cloned()
            .ok_or(StreamingError::InvalidCredentials)?;
        if user.password_hash != hash_password(password) {
            return Err(StreamingError::InvalidCredentials);
        }
        Ok(user)
    }

    pub fn record_play(&self, user_id: Uuid, song_id: Uuid) {
        write_guard(&self.history).entry(user_id).or_default().push(song_id);
    }

    pub fn history(&self, user_id: Uuid) -> Vec<Uuid> {
        read_guard(&self.history).get(&user_id).cloned().unwrap_or_default()
    }
}

impl Default for UserManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------- Playlists ----------

#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub song_ids: Vec<Uuid>,
}

pub struct PlaylistManager {
    playlists: RwLock<HashMap<Uuid, Playlist>>,
}

impl PlaylistManager {
    pub fn new() -> Self {
        Self {
            playlists: RwLock::new(HashMap::new()),
        }
    }

    pub fn create(&self, owner_id: Uuid, name: impl Into<String>) -> Playlist {
        let playlist = Playlist {
            id: Uuid::new_v4(),
            owner_id,
            name: name.into(),
            song_ids: Vec::new(),
        };
        write_guard(&self.playlists).insert(playlist.id, playlist.clone());
        playlist
    }

    pub fn add_song(
        &self,
        playlist_id: Uuid,
        actor_id: Uuid,
        song_id: Uuid,
    ) -> StreamingResult<()> {
        let mut playlists = write_guard(&self.playlists);
        let playlist = playlists
            .get_mut(&playlist_id)
            .ok_or(StreamingError::PlaylistNotFound)?;
        if playlist.owner_id != actor_id {
            return Err(StreamingError::PermissionDenied);
        }
        if !playlist.song_ids.contains(&song_id) {
            playlist.song_ids.push(song_id);
        }
        Ok(())
    }

    pub fn remove_song(
        &self,
        playlist_id: Uuid,
        actor_id: Uuid,
        song_id: Uuid,
    ) -> StreamingResult<()> {
        let mut playlists = write_guard(&self.playlists);
        let playlist = playlists
            .get_mut(&playlist_id)
            .ok_or(StreamingError::PlaylistNotFound)?;
        if playlist.owner_id != actor_id {
            return Err(StreamingError::PermissionDenied);
        }
        playlist.song_ids.retain(|id| *id != song_id);
        Ok(())
    }

    pub fn get(&self, playlist_id: Uuid) -> StreamingResult<Playlist> {
        read_guard(&self.playlists)
            .get(&playlist_id)
            .cloned()
            .ok_or(StreamingError::PlaylistNotFound)
    }

    pub fn for_owner(&self, owner_id: Uuid) -> Vec<Playlist> {
        read_guard(&self.playlists)
            .values()
            .filter(|p| p.owner_id == owner_id)
            .cloned()
            .collect()
    }
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------- Playback ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone)]
struct PlayerSession {
    queue: Vec<Uuid>,
    position_in_queue: usize,
    elapsed_secs: u32,
    state: PlaybackState,
}

/// One playback session per user. A HashMap<user_id, session> behind a
/// single RwLock is enough at this scale: sessions are cheap and short-lived,
/// unlike the movie/flight seat maps this pattern is borrowed from there's
/// no cross-user contention to guard against here.
pub struct MusicPlayer<'a> {
    library: &'a MusicLibrary,
    sessions: RwLock<HashMap<Uuid, PlayerSession>>,
}

impl<'a> MusicPlayer<'a> {
    pub fn new(library: &'a MusicLibrary) -> Self {
        Self {
            library,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn play_queue(&self, user_id: Uuid, queue: Vec<Uuid>) -> StreamingResult<()> {
        if let Some(&first) = queue.first() {
            self.library.song(first)?; // validate before starting
        }
        write_guard(&self.sessions).insert(
            user_id,
            PlayerSession {
                queue,
                position_in_queue: 0,
                elapsed_secs: 0,
                state: PlaybackState::Playing,
            },
        );
        Ok(())
    }

    pub fn pause(&self, user_id: Uuid) -> StreamingResult<()> {
        self.with_session(user_id, |s| s.state = PlaybackState::Paused)
    }

    pub fn resume(&self, user_id: Uuid) -> StreamingResult<()> {
        self.with_session(user_id, |s| s.state = PlaybackState::Playing)
    }

    pub fn skip(&self, user_id: Uuid) -> StreamingResult<Option<Uuid>> {
        let mut sessions = write_guard(&self.sessions);
        let session = sessions
            .get_mut(&user_id)
            .ok_or(StreamingError::NothingPlaying)?;
        session.position_in_queue += 1;
        session.elapsed_secs = 0;
        Ok(session.queue.get(session.position_in_queue).copied())
    }

    pub fn seek(&self, user_id: Uuid, position_secs: u32) -> StreamingResult<()> {
        let current = self.current_song(user_id)?;
        if position_secs > current.duration_secs {
            return Err(StreamingError::SeekOutOfBounds(
                position_secs,
                current.duration_secs,
            ));
        }
        self.with_session(user_id, |s| s.elapsed_secs = position_secs)
    }

    pub fn current_song(&self, user_id: Uuid) -> StreamingResult<Song> {
        let song_id = {
            let sessions = read_guard(&self.sessions);
            let session = sessions.get(&user_id).ok_or(StreamingError::NothingPlaying)?;
            *session
                .queue
                .get(session.position_in_queue)
                .ok_or(StreamingError::NothingPlaying)?
        };
        self.library.song(song_id)
    }

    pub fn state(&self, user_id: Uuid) -> Option<PlaybackState> {
        read_guard(&self.sessions).get(&user_id).map(|s| s.state)
    }

    fn with_session(
        &self,
        user_id: Uuid,
        f: impl FnOnce(&mut PlayerSession),
    ) -> StreamingResult<()> {
        let mut sessions = write_guard(&self.sessions);
        let session = sessions
            .get_mut(&user_id)
            .ok_or(StreamingError::NothingPlaying)?;
        f(session);
        Ok(())
    }
}

// ---------- Recommendations ----------

/// Genre-affinity recommender: counts genres in the user's play history,
/// then ranks unplayed songs by how often their genre appears there.
/// Good enough for a demo; a real system would use collaborative filtering.
pub struct MusicRecommender<'a> {
    library: &'a MusicLibrary,
}

impl<'a> MusicRecommender<'a> {
    pub fn new(library: &'a MusicLibrary) -> Self {
        Self { library }
    }

    pub fn recommend(&self, history: &[Uuid], limit: usize) -> Vec<Song> {
        let played: std::collections::HashSet<Uuid> = history.iter().copied().collect();
        let mut genre_counts: HashMap<String, u32> = HashMap::new();
        for song_id in history {
            if let Ok(song) = self.library.song(*song_id) {
                *genre_counts.entry(song.genre).or_insert(0) += 1;
            }
        }

        let mut candidates: Vec<(u32, Song)> = read_guard(&self.library.songs)
            .values()
            .filter(|song| !played.contains(&song.id))
            .map(|song| {
                let score = genre_counts.get(&song.genre).copied().unwrap_or(0);
                (score, song.clone())
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.title.cmp(&b.1.title)));
        candidates.into_iter().take(limit).map(|(_, s)| s).collect()
    }
}

// ---------- Composition root ----------

/// Composition root, following the same shape as
/// MovieTicketBookingSystem/AirlineManagementSystem: shared components built
/// once and handed out via Arc, no process-global state.
pub struct MusicStreamingService {
    pub library: Arc<MusicLibrary>,
    pub users: Arc<UserManager>,
    pub playlists: Arc<PlaylistManager>,
}

impl MusicStreamingService {
    pub fn new() -> Self {
        Self {
            library: Arc::new(MusicLibrary::new()),
            users: Arc::new(UserManager::new()),
            playlists: Arc::new(PlaylistManager::new()),
        }
    }

    /// The process-wide default instance, for demo convenience only.
    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<MusicStreamingService>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(MusicStreamingService::new()))
    }
}

impl Default for MusicStreamingService {
    fn default() -> Self {
        Self::new()
    }
}

fn run_demo() {
    let service = MusicStreamingService::new();

    let artist = service.library.add_artist("Coldplay");
    let album = service.library.add_album("Parachutes", artist.id);
    let yellow = service
        .library
        .add_song("Yellow", artist.id, album.id, 269, "Rock");
    let trouble = service
        .library
        .add_song("Trouble", artist.id, album.id, 275, "Rock");
    let jazz_artist = service.library.add_artist("Miles Davis");
    let jazz_album = service.library.add_album("Kind of Blue", jazz_artist.id);
    let so_what = service
        .library
        .add_song("So What", jazz_artist.id, jazz_album.id, 561, "Jazz");

    println!("=== Search 'yellow' ===");
    for song in service.library.search("yellow") {
        println!("  {} ({}s)", song.title, song.duration_secs);
    }

    let alice = service.users.register("alice", "hunter2").expect("registered");
    service
        .users
        .login("alice", "hunter2")
        .expect("login succeeds");
    match service.users.login("alice", "wrong") {
        Err(StreamingError::InvalidCredentials) => println!("\nbad password rejected"),
        other => panic!("expected InvalidCredentials, got {other:?}"),
    }

    println!("\n=== Playlist ===");
    let playlist = service.playlists.create(alice.id, "Chill");
    service
        .playlists
        .add_song(playlist.id, alice.id, yellow.id)
        .expect("added");
    service
        .playlists
        .add_song(playlist.id, alice.id, trouble.id)
        .expect("added");
    let stored = service.playlists.get(playlist.id).expect("exists");
    println!("  {} has {} songs", stored.name, stored.song_ids.len());

    println!("\n=== Playback ===");
    let player = MusicPlayer::new(&service.library);
    player
        .play_queue(alice.id, vec![yellow.id, trouble.id])
        .expect("plays");
    println!(
        "  now playing: {} [{}]",
        player.current_song(alice.id).unwrap().title,
        player.state(alice.id).unwrap()
    );
    player.seek(alice.id, 30).expect("seeks");
    player.pause(alice.id).expect("pauses");
    println!("  state after pause: {}", player.state(alice.id).unwrap());
    let next = player.skip(alice.id).expect("skips");
    println!(
        "  skipped to: {}",
        service.library.song(next.unwrap()).unwrap().title
    );
    service.users.record_play(alice.id, yellow.id);
    service.users.record_play(alice.id, trouble.id);

    println!("\n=== Recommendations ===");
    let recommender = MusicRecommender::new(&service.library);
    let history = service.users.history(alice.id);
    for song in recommender.recommend(&history, 3) {
        println!("  {} [{}]", song.title, song.genre);
    }
    let _ = so_what; // seeded for the recommender to have a jazz candidate
}

fn main() {
    run_demo();
    println!("\nAll done.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> MusicStreamingService {
        MusicStreamingService::new()
    }

    #[test]
    fn test_duplicate_username_rejected() {
        let svc = service();
        svc.users.register("bob", "pw").expect("first ok");
        let result = svc.users.register("bob", "pw2");
        assert!(matches!(result, Err(StreamingError::UsernameTaken(_))));
    }

    #[test]
    fn test_login_wrong_password() {
        let svc = service();
        svc.users.register("bob", "pw").expect("registered");
        let result = svc.users.login("bob", "wrong");
        assert!(matches!(result, Err(StreamingError::InvalidCredentials)));
    }

    #[test]
    fn test_search_matches_title_and_genre() {
        let svc = service();
        let artist = svc.library.add_artist("A");
        let album = svc.library.add_album("Al", artist.id);
        svc.library.add_song("Blue Skies", artist.id, album.id, 200, "Pop");
        svc.library.add_song("Night Drive", artist.id, album.id, 180, "Synthwave");

        assert_eq!(svc.library.search("blue").len(), 1);
        assert_eq!(svc.library.search("pop").len(), 1);
        assert!(svc.library.search("nonexistent").is_empty());
    }

    #[test]
    fn test_playlist_permission_denied_for_non_owner() {
        let svc = service();
        let owner = svc.users.register("owner", "pw").unwrap();
        let stranger = svc.users.register("stranger", "pw").unwrap();
        let artist = svc.library.add_artist("A");
        let album = svc.library.add_album("Al", artist.id);
        let song = svc.library.add_song("S", artist.id, album.id, 100, "Pop");
        let playlist = svc.playlists.create(owner.id, "Mix");

        let result = svc.playlists.add_song(playlist.id, stranger.id, song.id);
        assert!(matches!(result, Err(StreamingError::PermissionDenied)));
    }

    #[test]
    fn test_player_lifecycle() {
        let svc = service();
        let user = svc.users.register("u", "pw").unwrap();
        let artist = svc.library.add_artist("A");
        let album = svc.library.add_album("Al", artist.id);
        let s1 = svc.library.add_song("One", artist.id, album.id, 100, "Pop");
        let s2 = svc.library.add_song("Two", artist.id, album.id, 100, "Pop");

        let player = MusicPlayer::new(&svc.library);
        player.play_queue(user.id, vec![s1.id, s2.id]).unwrap();
        assert_eq!(player.current_song(user.id).unwrap().id, s1.id);

        player.pause(user.id).unwrap();
        assert_eq!(player.state(user.id).unwrap(), PlaybackState::Paused);
        player.resume(user.id).unwrap();
        assert_eq!(player.state(user.id).unwrap(), PlaybackState::Playing);

        let next = player.skip(user.id).unwrap();
        assert_eq!(next, Some(s2.id));
        assert_eq!(player.current_song(user.id).unwrap().id, s2.id);
    }

    #[test]
    fn test_seek_out_of_bounds_rejected() {
        let svc = service();
        let user = svc.users.register("u", "pw").unwrap();
        let artist = svc.library.add_artist("A");
        let album = svc.library.add_album("Al", artist.id);
        let s1 = svc.library.add_song("One", artist.id, album.id, 100, "Pop");

        let player = MusicPlayer::new(&svc.library);
        player.play_queue(user.id, vec![s1.id]).unwrap();
        let result = player.seek(user.id, 200);
        assert!(matches!(result, Err(StreamingError::SeekOutOfBounds(200, 100))));
    }

    #[test]
    fn test_recommender_prefers_played_genre_and_excludes_played_songs() {
        let svc = service();
        let artist = svc.library.add_artist("A");
        let album = svc.library.add_album("Al", artist.id);
        let rock1 = svc.library.add_song("R1", artist.id, album.id, 100, "Rock");
        let rock2 = svc.library.add_song("R2", artist.id, album.id, 100, "Rock");
        let jazz1 = svc.library.add_song("J1", artist.id, album.id, 100, "Jazz");

        let recommender = MusicRecommender::new(&svc.library);
        let history = vec![rock1.id];
        let recs = recommender.recommend(&history, 5);

        assert!(!recs.iter().any(|s| s.id == rock1.id), "played song excluded");
        assert_eq!(recs[0].id, rock2.id, "unplayed rock ranked above jazz");
        assert!(recs.iter().any(|s| s.id == jazz1.id));
    }
}
