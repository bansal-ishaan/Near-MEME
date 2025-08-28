use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use schemars::JsonSchema;
use near_sdk::{
    env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault,
    collections::{LookupMap, UnorderedSet, Vector},
    serde::{Deserialize, Serialize},
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MemeFiContract {
    memes: LookupMap<String, MemeNFT>,
    all_memes: Vector<String>,
    likes: LookupMap<String, UnorderedSet<String>>,
    comments: LookupMap<String, Vector<Comment>>,
    listings: LookupMap<String, u128>,
    user_stats: LookupMap<String, UserStats>,
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Memes,
    AllMemes,
    Likes,
    Comments,
    CommentsInner { meme_id: String },
    Listings,
    UserStats,
    LikeSet { meme_id: String },
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct MemeNFT {
    pub id: String,
    pub owner_id: String,
    pub creator_id: String,
    pub media_url: String,
    pub title: String,
    pub description: String,
    pub royalty: u8,
    pub likes_count: u32,
    pub comments_count: u32,
    pub last_like_timestamp: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct Comment {
    pub user_id: String,
    pub text: String,
    pub timestamp: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Default, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct UserStats {
    pub total_likes: u32,
    pub total_comments: u32,
    pub total_earnings: u128,
}

#[near_bindgen]
impl MemeFiContract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Contract already initialized");
        Self {
            memes: LookupMap::new(StorageKey::Memes),
            all_memes: Vector::new(StorageKey::AllMemes),
            likes: LookupMap::new(StorageKey::Likes),
            comments: LookupMap::new(StorageKey::Comments),
            listings: LookupMap::new(StorageKey::Listings),
            user_stats: LookupMap::new(StorageKey::UserStats),
        }
    }

    pub fn mint_meme(
        &mut self,
        id: String,
        media_url: String,
        title: String,
        description: String,
        royalty: u8,
    ) {
        let creator = env::predecessor_account_id().to_string();
        assert!(royalty <= 100, "Royalty must be between 0 and 100");
        assert!(
            !self.memes.contains_key(&id),
            "Meme ID already exists"
        );

        let meme = MemeNFT {
            id: id.clone(),
            owner_id: creator.clone(),
            creator_id: creator,
            media_url,
            title,
            description,
            royalty,
            likes_count: 0,
            comments_count: 0,
            last_like_timestamp: 0,
        };
        self.memes.insert(&id, &meme);
        self.all_memes.push(&id);
    }

    pub fn get_meme(&self, id: String) -> Option<MemeNFT> {
        self.memes.get(&id)
    }

    pub fn get_user_memes(&self, user_id: String) -> Vec<MemeNFT> {
        let mut result = Vec::new();
        for meme_id in self.all_memes.iter() {
            if let Some(meme) = self.memes.get(&meme_id) {
                if meme.owner_id == user_id {
                    result.push(meme);
                }
            }
        }
        result
    }

    pub fn get_all_memes(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<MemeNFT> {
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(50).min(100);
        let mut result = Vec::new();
        for i in from_index..std::cmp::min(from_index + limit, self.all_memes.len()) {
            if let Some(meme_id) = self.all_memes.get(i) {
                if let Some(meme) = self.memes.get(&meme_id) {
                    result.push(meme);
                }
            }
        }
        result
    }

    pub fn get_memes_count(&self) -> u64 {
        self.all_memes.len()
    }

    pub fn like_meme(&mut self, meme_id: String) {
        let liker = env::predecessor_account_id().to_string();
        let mut meme = self.memes.get(&meme_id).expect("Meme not found");

        let mut liked_users = self.likes.get(&meme_id).unwrap_or_else(|| {
            UnorderedSet::new(StorageKey::LikeSet {
                meme_id: meme_id.clone(),
            })
        });

        if liked_users.contains(&liker) {
            env::panic_str("User already liked this meme");
        }
        liked_users.insert(&liker);
        self.likes.insert(&meme_id, &liked_users);

        meme.likes_count += 1;
        meme.last_like_timestamp = env::block_timestamp();
        self.memes.insert(&meme_id, &meme);

        let mut stats = self.user_stats.get(&meme.owner_id).unwrap_or_default();
        stats.total_likes += 1;
        self.user_stats.insert(&meme.owner_id, &stats);
    }

    pub fn unlike_meme(&mut self, meme_id: String) {
        let user = env::predecessor_account_id().to_string();
        let mut meme = self.memes.get(&meme_id).expect("Meme not found");

        let mut liked_users = self.likes.get(&meme_id).expect("No likes found for meme");

        if !liked_users.contains(&user) {
            env::panic_str("User has not liked this meme");
        }
        liked_users.remove(&user);
        self.likes.insert(&meme_id, &liked_users);

        meme.likes_count = meme.likes_count.saturating_sub(1);
        self.memes.insert(&meme_id, &meme);

        let mut stats = self.user_stats.get(&meme.owner_id).unwrap_or_default();
        stats.total_likes = stats.total_likes.saturating_sub(1);
        self.user_stats.insert(&meme.owner_id, &stats);
    }

    pub fn comment_meme(&mut self, meme_id: String, text: String) {
        let commenter = env::predecessor_account_id().to_string();
        let mut meme = self.memes.get(&meme_id).expect("Meme not found");

        assert!(!text.trim().is_empty(), "Comment text cannot be empty");
        assert!(text.len() <= 500, "Comment text too long (max 500 characters)");

        let mut comments = self.comments.get(&meme_id).unwrap_or_else(|| {
            Vector::new(StorageKey::CommentsInner { meme_id: meme_id.clone() })
        });

        let comment = Comment {
            user_id: commenter.clone(),
            text: text.trim().to_string(),
            timestamp: env::block_timestamp(),
        };
        comments.push(&comment);
        self.comments.insert(&meme_id, &comments);

        meme.comments_count += 1;
        self.memes.insert(&meme_id, &meme);

        let mut stats = self.user_stats.get(&meme.owner_id).unwrap_or_default();
        stats.total_comments += 1;
        self.user_stats.insert(&meme.owner_id, &stats);
    }

    pub fn get_comments(&self, meme_id: String) -> Vec<Comment> {
        if let Some(comments) = self.comments.get(&meme_id) {
            comments.to_vec()
        } else {
            vec![]
        }
    }

    pub fn get_likes(&self, meme_id: String) -> u32 {
        if let Some(set) = self.likes.get(&meme_id) {
            set.len() as u32
        } else {
            0
        }
    }
        }
