# Vector Database for Element Blockchain

## 💡 The Idea

**Use a vector database to store element blockchains with semantic search capabilities!**

Instead of:
- SQLite: Key-value lookup only
- Files: Linear scan only

Get:
- **Similarity search**: "Find configs similar to this one"
- **Semantic queries**: "Find all network changes"
- **Anomaly detection**: "Which configs are unusual?"
- **Clustering**: "Group similar modifications"

## 🎯 What is a Vector Database?

### Traditional Database
```sql
SELECT * FROM elements WHERE id = 'interface:eth0'
-- Returns: Exact match only
```

### Vector Database
```rust
// Store element as vector (embedding)
let element_vector = embed(element_blockchain);
db.insert(element_id, element_vector, metadata);

// Similarity search
let similar = db.search_similar(query_vector, top_k=10);
// Returns: Top 10 similar elements!
```

### How It Works

```
Element State → Embedding Model → Vector (768 dimensions)
  {                                [0.23, -0.45, 0.67, ...]
    "bridge": "ovsbr0",    →         ↓
    "vmid": 101,                  Vector DB
    "mtu": 1500                   (similarity search)
  }                                  ↓
                              Find similar configs!
```

## 🔍 Use Cases for Blockchain

### 1. **Find Similar Configurations**

```rust
// Find interfaces with similar config to eth0
let eth0_blockchain = get_blockchain("interface:eth0")?;
let eth0_vector = embed(&eth0_blockchain.current_state())?;

let similar = vector_db.search_similar(
    eth0_vector,
    top_k: 10,
    filter: "type = 'NetworkInterface'"
)?;

// Returns:
// - eth1 (95% similar - same bridge, different IP)
// - wlan0 (87% similar - same MTU, different bridge)
// - eth2 (82% similar - similar config)
```

**Use case:** Configuration deduplication, finding redundant elements

---

### 2. **Semantic Search**

```rust
// "Find all bridge changes"
let query = "bridge modification";
let query_vector = embed_text(query)?;

let results = vector_db.search_similar(query_vector, top_k: 20)?;

// Returns all blockchain entries where bridge was modified
// WITHOUT needing exact keyword matching!
```

**Use case:** Natural language queries over blockchain history

---

### 3. **Anomaly Detection**

```rust
// Find unusual configurations
let all_vectors = vector_db.get_all_vectors("NetworkInterface")?;
let mean_vector = calculate_mean(all_vectors);

// Find elements far from mean (outliers)
let outliers = vector_db.search_farthest(mean_vector, top_k: 10)?;

// Returns:
// - eth99 (unusual MTU: 9000 vs typical 1500)
// - test-if (unusual bridge: testbr0 vs typical ovsbr0)
```

**Use case:** Security - detect tampered or misconfigured elements

---

### 4. **Configuration Drift Detection**

```rust
// Compare current state to baseline
let baseline_vector = embed(&baseline_config)?;
let current_vectors = vector_db.get_all_vectors("NetworkInterface")?;

for (id, vec) in current_vectors {
    let similarity = cosine_similarity(baseline_vector, vec);
    if similarity < 0.9 {
        println!("Drift detected in {}: {}% different", id, (1.0 - similarity) * 100);
    }
}
```

**Use case:** Configuration management, compliance checking

---

### 5. **Smart Deduplication**

```rust
// Find near-duplicates (fuzzy matching)
let new_config_vector = embed(&new_config)?;

let duplicates = vector_db.search_similar(
    new_config_vector,
    similarity_threshold: 0.95  // 95% similar or more
)?;

if !duplicates.is_empty() {
    println!("Similar config already exists: {:?}", duplicates);
    // Reuse existing or warn user
}
```

**Use case:** Prevent creating nearly identical elements

---

## 🏗️ Architecture

### Vector Database Options

| Database | Language | Features | Best For |
|----------|----------|----------|----------|
| **Qdrant** | Rust | Fast, local, HTTP API | ✅ Perfect fit! |
| **Milvus** | C++/Go | Scalable, distributed | Large scale |
| **Weaviate** | Go | GraphQL, modules | Complex queries |
| **ChromaDB** | Python | Simple, embedded | Prototyping |
| **Lance** | Rust | Columnar, fast | Analytics |

**Recommendation: Qdrant** (it's Rust, fast, and can be embedded!)

### Hybrid Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Central Ledger (Index)                 │
│              /var/lib/ledger.jsonl                      │
│           Global timeline, references                   │
└─────────────────────────────────────────────────────────┘
                            ↓
                    ┌───────┴───────┐
                    ↓               ↓
        ┌───────────────────┐  ┌──────────────────────┐
        │  SQLite Database  │  │  Vector Database     │
        │  (Fast Lookup)    │  │  (Similarity Search) │
        ├───────────────────┤  ├──────────────────────┤
        │ ID → Blockchain   │  │ Vector → Similar IDs │
        │ O(1) exact match  │  │ O(log n) similarity  │
        └───────────────────┘  └──────────────────────┘
                    ↓               ↓
            ┌───────────────────────────┐
            │   Unified Query Layer     │
            │                           │
            │ - Exact: Use SQLite       │
            │ - Similar: Use Vector DB  │
            │ - Timeline: Use Ledger    │
            └───────────────────────────┘
```

---

## 💻 Implementation with Qdrant

### Setup

```toml
[dependencies]
qdrant-client = "1.7"
serde_json = "1.0"
```

### Code

```rust
use qdrant_client::{
    client::QdrantClient,
    qdrant::{
        vectors_config::Config, CreateCollection, Distance, PointStruct, 
        SearchPoints, VectorParams, VectorsConfig,
    },
};

pub struct VectorBlockchainStore {
    client: QdrantClient,
    collection_name: String,
}

impl VectorBlockchainStore {
    pub async fn new(url: &str, collection: &str) -> Result<Self> {
        let client = QdrantClient::from_url(url).build()?;
        
        // Create collection with 768-dimensional vectors (BERT embeddings)
        client.create_collection(&CreateCollection {
            collection_name: collection.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: 768,  // Embedding dimension
                    distance: Distance::Cosine.into(),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        }).await?;
        
        Ok(Self {
            client,
            collection_name: collection.to_string(),
        })
    }
    
    /// Store element blockchain with vector embedding
    pub async fn store_blockchain(
        &self,
        element_id: &str,
        blockchain: &ElementBlockchain,
        embedding: Vec<f32>,  // 768-dim vector
    ) -> Result<()> {
        // Create point with vector + metadata
        let point = PointStruct::new(
            element_id.to_string(),
            embedding,
            serde_json::json!({
                "element_id": element_id,
                "type": blockchain.element_type,
                "current_hash": blockchain.current_hash,
                "height": blockchain.blocks.len(),
                "blockchain_json": serde_json::to_string(blockchain)?,
            }),
        );
        
        self.client.upsert_points(
            &self.collection_name,
            None,
            vec![point],
            None
        ).await?;
        
        Ok(())
    }
    
    /// Find similar elements
    pub async fn find_similar(
        &self,
        query_vector: Vec<f32>,
        top_k: usize,
    ) -> Result<Vec<(String, f32, ElementBlockchain)>> {
        let search_result = self.client.search_points(&SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_vector,
            limit: top_k as u64,
            with_payload: Some(true.into()),
            ..Default::default()
        }).await?;
        
        let mut results = Vec::new();
        for point in search_result.result {
            let element_id = point.id.unwrap().to_string();
            let score = point.score;
            let blockchain_json = point.payload.get("blockchain_json")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("No blockchain data"))?;
            let blockchain: ElementBlockchain = serde_json::from_str(blockchain_json)?;
            
            results.push((element_id, score, blockchain));
        }
        
        Ok(results)
    }
    
    /// Semantic search with text query
    pub async fn semantic_search(
        &self,
        query_text: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32, ElementBlockchain)>> {
        // Convert text to vector using embedding model
        let query_vector = self.embed_text(query_text).await?;
        self.find_similar(query_vector, top_k).await
    }
    
    /// Detect anomalies (outliers)
    pub async fn detect_anomalies(
        &self,
        element_type: &str,
        threshold: f32,
    ) -> Result<Vec<String>> {
        // Get all vectors for this type
        let all_points = self.client.scroll(&ScrollPoints {
            collection_name: self.collection_name.clone(),
            filter: Some(Filter {
                must: vec![Condition {
                    field: "type".to_string(),
                    r#match: Some(Match::Keyword(element_type.to_string())),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            with_vectors: Some(true.into()),
            ..Default::default()
        }).await?;
        
        // Calculate mean vector
        let vectors: Vec<Vec<f32>> = all_points.result.iter()
            .map(|p| p.vector.clone())
            .collect();
        let mean_vector = calculate_mean_vector(&vectors);
        
        // Find points far from mean
        let outliers = self.client.search_points(&SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: mean_vector,
            limit: 100,
            score_threshold: Some(1.0 - threshold),  // Low similarity = outlier
            ..Default::default()
        }).await?;
        
        Ok(outliers.result.iter()
            .map(|p| p.id.unwrap().to_string())
            .collect())
    }
    
    /// Generate embedding for text (using local model)
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        // Use sentence-transformers or similar
        // For example: all-MiniLM-L6-v2 (384-dim) or BERT (768-dim)
        
        use rust_bert::pipelines::sentence_embeddings::*;
        
        let model = SentenceEmbeddingsBuilder::remote(
            SentenceEmbeddingsModelType::AllMiniLmL12V2
        ).create_model()?;
        
        let embeddings = model.encode(&[text])?;
        Ok(embeddings[0].clone())
    }
    
    /// Generate embedding for blockchain state
    fn embed_blockchain(&self, blockchain: &ElementBlockchain) -> Result<Vec<f32>> {
        // Convert blockchain to text representation
        let state_text = serde_json::to_string(blockchain.current_state()?)?;
        self.embed_text(&state_text).await
    }
}

fn calculate_mean_vector(vectors: &[Vec<f32>]) -> Vec<f32> {
    let dim = vectors[0].len();
    let mut mean = vec![0.0; dim];
    
    for vec in vectors {
        for (i, &val) in vec.iter().enumerate() {
            mean[i] += val;
        }
    }
    
    let count = vectors.len() as f32;
    mean.iter_mut().for_each(|v| *v /= count);
    
    mean
}
```

---

## 🎯 Usage Examples

### Example 1: Find Similar Configs

```rust
let vector_store = VectorBlockchainStore::new("http://localhost:6334", "elements").await?;

// Store element with embedding
let eth0 = get_blockchain("interface:eth0")?;
let eth0_embedding = vector_store.embed_blockchain(&eth0)?;
vector_store.store_blockchain("interface:eth0", &eth0, eth0_embedding).await?;

// Find similar interfaces
let similar = vector_store.find_similar(eth0_embedding, 5).await?;

for (id, similarity, blockchain) in similar {
    println!("{}: {:.2}% similar", id, similarity * 100.0);
}

// Output:
// interface:eth1: 95.3% similar (same bridge, different IP)
// interface:eth2: 89.7% similar (same MTU, different bridge)
// interface:wlan0: 78.2% similar (similar settings)
```

---

### Example 2: Semantic Search

```rust
// Natural language query!
let results = vector_store.semantic_search(
    "interfaces that were moved to a different bridge",
    10
).await?;

// Returns all interfaces where bridge was modified
// WITHOUT needing exact keyword "bridge" in the data!
```

---

### Example 3: Anomaly Detection

```rust
// Find unusual network interfaces
let anomalies = vector_store.detect_anomalies(
    "NetworkInterface",
    0.7  // 70% similarity threshold
).await?;

println!("Unusual interfaces detected: {:?}", anomalies);

// Output:
// ["interface:test99"]  ← Has MTU 9000 (everyone else has 1500)
// ["interface:debug0"]  ← Has unusual bridge name
```

---

### Example 4: Configuration Deduplication

```rust
// Before creating new interface
let new_config = InterfaceConfig { bridge: "ovsbr0", vmid: 101, ... };
let new_embedding = vector_store.embed_text(&format!("{:?}", new_config))?;

let duplicates = vector_store.find_similar(new_embedding, 1).await?;

if let Some((id, similarity, _)) = duplicates.first() {
    if similarity > &0.95 {
        println!("⚠️  Very similar config exists: {} ({:.0}% similar)", 
                 id, similarity * 100.0);
        println!("Consider reusing existing element");
    }
}
```

---

## 📊 Performance Comparison

| Operation | SQLite | Vector DB | Use Case |
|-----------|--------|-----------|----------|
| Exact lookup | 0.1ms ✅ | 1-2ms | Get blockchain by ID |
| Find similar | N/A | 5-10ms ✅ | Find similar configs |
| Semantic search | N/A | 10-20ms ✅ | Natural language queries |
| Anomaly detection | Complex query | 20-50ms ✅ | Find outliers |
| Full scan | 100ms+ | 50ms ✅ | Analyze all elements |

**Conclusion:** SQLite for exact lookups, Vector DB for intelligent queries!

---

## 🎨 Combined Architecture

### Best of All Worlds

```rust
pub struct HybridBlockchainStore {
    // Tier 1: Central ledger (timeline)
    ledger: BlockchainLedger,
    
    // Tier 2: SQLite (fast exact lookup)
    sqlite: UniversalElementStore,
    
    // Tier 3: Vector DB (similarity search)
    vector_db: VectorBlockchainStore,
}

impl HybridBlockchainStore {
    /// Store element in all tiers
    pub async fn store_element(
        &self,
        element_id: &str,
        blockchain: &ElementBlockchain,
    ) -> Result<()> {
        // 1. Store in SQLite (fast lookup)
        self.sqlite.store_blockchain(element_id, &blockchain)?;
        
        // 2. Generate embedding and store in vector DB (similarity)
        let embedding = self.vector_db.embed_blockchain(&blockchain)?;
        self.vector_db.store_blockchain(element_id, &blockchain, embedding).await?;
        
        // 3. Record in central ledger (timeline)
        self.ledger.add_data("element_update", "stored", json!({
            "element_id": element_id,
            "hash": blockchain.current_hash,
        }))?;
        
        Ok(())
    }
    
    /// Query strategy selector
    pub async fn query(&self, query: Query) -> Result<Vec<ElementBlockchain>> {
        match query {
            // Exact ID → Use SQLite (fastest)
            Query::ById(id) => {
                vec![self.sqlite.load_blockchain(&id)?]
            }
            
            // Similarity → Use Vector DB
            Query::Similar { to, top_k } => {
                let blockchain = self.sqlite.load_blockchain(&to)?;
                let embedding = self.vector_db.embed_blockchain(&blockchain)?;
                let results = self.vector_db.find_similar(embedding, top_k).await?;
                Ok(results.into_iter().map(|(_, _, bc)| bc).collect())
            }
            
            // Semantic → Use Vector DB
            Query::Semantic { text, top_k } => {
                let results = self.vector_db.semantic_search(&text, top_k).await?;
                Ok(results.into_iter().map(|(_, _, bc)| bc).collect())
            }
            
            // Timeline → Use Central Ledger
            Query::TimeRange { start, end } => {
                let blocks = self.ledger.get_blocks_in_time_range(start, end)?;
                // ... fetch blockchains from SQLite
            }
        }
    }
}
```

---

## 🚀 Killer Features

### 1. Configuration Recommendations
```rust
// "What should I configure eth0 like?"
let recommendations = vector_store.find_similar(
    current_eth0_state,
    top_k: 3
).await?;

println!("Similar interfaces you might want to configure like:");
for (id, similarity, blockchain) in recommendations {
    println!("- {}: {:.0}% similar", id, similarity * 100.0);
}
```

### 2. Change Impact Analysis
```rust
// "Which other elements will be affected if I change this?"
let eth0_vector = get_embedding("interface:eth0")?;
let affected = vector_store.find_similar(eth0_vector, 20).await?;

println!("Elements that might be affected by changing eth0:");
for (id, similarity, _) in affected {
    if similarity > 0.8 {
        println!("- {} ({:.0}% similar config)", id, similarity * 100.0);
    }
}
```

### 3. Smart Rollback
```rust
// "Find a stable config similar to current but from last week"
let current_vector = get_current_embedding("interface:eth0")?;

let candidates = vector_store.find_similar(current_vector, 100).await?;
let last_week = candidates.into_iter()
    .filter(|(_, _, bc)| bc.created_at < one_week_ago())
    .max_by_key(|(_, similarity, _)| similarity)
    .unwrap();

println!("Safest rollback target: {} ({:.0}% similar)", 
         last_week.0, last_week.1 * 100.0);
```

---

## ✅ Recommendation

**Use TRIPLE-TIER architecture:**

```
1. SQLite → Fast exact lookups (0.1ms)
2. Vector DB → Intelligent queries (5-20ms)
3. Central Ledger → Global timeline
```

**When to use Vector DB:**
- ✅ Finding similar configurations
- ✅ Semantic/natural language search
- ✅ Anomaly/drift detection
- ✅ Configuration recommendations
- ✅ Smart deduplication

**When NOT to use Vector DB:**
- ❌ Simple ID lookups (use SQLite)
- ❌ Timeline queries (use central ledger)
- ❌ Exact matches (use SQLite)

**Dependencies:**
```toml
qdrant-client = "1.7"      # Vector database
rusqlite = "0.30"          # SQLite
rust-bert = "0.21"         # Embeddings
```

**This gives you AI-powered blockchain queries!** 🤖✨
