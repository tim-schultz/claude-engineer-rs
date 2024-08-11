#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use mockall::mock;
    use mockall::predicate::*;

    mock! {
        Bert {}
        #[async_trait::async_trait]
        impl Embedding for Bert {
            async fn generate_embedding(&self, prompt: String) -> Result<Vec<f32>>;
            async fn generate_embeddings(&self, prompts: Vec<String>) -> Result<Vec<Vec<f32>>>;
        }
    }

    mock! {
        Qdrant {}
        impl Qdrant {
            fn new(url: &str) -> Result<Self>;
            async fn insert_many(&self, collection_name: &str, vectors: Vec<Vec<f32>>, records: Vec<Record>) -> Result<()>;
            async fn search(&self, collection_name: &str, vector: Vec<f32>, limit: u64, filter: Option<String>) -> Result<Vec<FoundPoint>>;
        }
    }

    mock! {
        HTML {}
        impl HTML {
            async fn from_url(url: &str) -> Result<Self>;
        }
    }

    #[tokio::test]
    async fn test_new() -> Result<()> {
        let collection_name = "test_collection".to_string();
        let rust_book_scraper = RustBookScraper::new(collection_name.clone()).await?;

        assert_eq!(rust_book_scraper.collection_name, collection_name);
        Ok(())
    }

    #[tokio::test]
    async fn test_scrape_and_insert() -> Result<()> {
        let mut mock_bert = MockBert::new();
        let mut mock_qdrant = MockQdrant::new();
        let mut mock_html = MockHTML::new();

        // Set up expectations
        mock_bert
            .expect_generate_embeddings()
            .returning(|_| Ok(vec![vec![0.1, 0.2, 0.3]]));

        mock_qdrant.expect_insert_many().returning(|_, _, _| Ok(()));

        mock_html.expect_from_url().returning(|_| {
            Ok(HTML {
                body: "<html><body><main>Test content</main></body></html>".to_string(),
            })
        });

        let rust_book_scraper = RustBookScraper {
            bert: mock_bert,
            qdrant: mock_qdrant,
            collection_name: "test_collection".to_string(),
        };

        assert!(rust_book_scraper.scrape_and_insert().await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_query_and_get_prompt() -> Result<()> {
        let mut mock_bert = MockBert::new();
        let mut mock_qdrant = MockQdrant::new();

        // Set up expectations
        mock_bert
            .expect_generate_embedding()
            .returning(|_| Ok(vec![0.1, 0.2, 0.3]));

        mock_qdrant.expect_search().returning(|_, _, _, _| {
            Ok(vec![
                FoundPoint {
                    id: 1,
                    payload: Some(serde_json::json!({"content": "Test content 1"})),
                    score: 0.9,
                    vector: None,
                },
                FoundPoint {
                    id: 2,
                    payload: Some(serde_json::json!({"content": "Test content 2"})),
                    score: 0.8,
                    vector: None,
                },
            ])
        });

        let rust_book_scraper = RustBookScraper {
            bert: mock_bert,
            qdrant: mock_qdrant,
            collection_name: "test_collection".to_string(),
        };

        let prompt = rust_book_scraper.query_and_get_prompt("Test query").await?;
        assert!(prompt.contains("Test query"));
        assert!(prompt.contains("Test content 1"));
        assert!(prompt.contains("Test content 2"));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_book_pages() -> Result<()> {
        let mut mock_html = MockHTML::new();

        // Set up expectations
        mock_html.expect_from_url()
            .returning(|_| Ok(HTML { body: r#"
                <html>
                    <body>
                        <ol class="chapter">
                            <li><a href="ch01-00-getting-started.html">Getting Started</a></li>
                            <li><a href="ch02-00-guessing-game-tutorial.html">Programming a Guessing Game</a></li>
                        </ol>
                    </body>
                </html>
            "#.to_string() }));

        let rust_book_scraper = RustBookScraper {
            bert: MockBert::new(),
            qdrant: MockQdrant::new(),
            collection_name: "test_collection".to_string(),
        };

        let pages = rust_book_scraper.get_book_pages().await?;
        assert_eq!(pages.len(), 2);
        assert!(pages[0].ends_with("ch01-00-getting-started.html"));
        assert!(pages[1].ends_with("ch02-00-guessing-game-tutorial.html"));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_page_html() -> Result<()> {
        let mut mock_html = MockHTML::new();

        // Set up expectations
        mock_html.expect_from_url().returning(|_| {
            Ok(HTML {
                body: "<html><body><main>Test content</main></body></html>".to_string(),
            })
        });

        let rust_book_scraper = RustBookScraper {
            bert: MockBert::new(),
            qdrant: MockQdrant::new(),
            collection_name: "test_collection".to_string(),
        };

        let html = rust_book_scraper
            .get_page_html("https://example.com")
            .await?;
        assert_eq!(
            html.select(&Selector::parse("main").unwrap())
                .next()
                .unwrap()
                .inner_html(),
            "Test content"
        );
        Ok(())
    }

    #[test]
    fn test_extract_content() {
        let rust_book_scraper = RustBookScraper {
            bert: MockBert::new(),
            qdrant: MockQdrant::new(),
            collection_name: "test_collection".to_string(),
        };

        let html = Html::parse_document(
            "<html><body><main><p>Test content 1</p><p>Test content 2</p></main></body></html>",
        );
        let content = rust_book_scraper.extract_content(&html);
        assert_eq!(content, "Test content 1 Test content 2");
    }
}
