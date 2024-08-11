use anyhow::Result;
use orca_core::{
    llm::{bert::Bert, Embedding},
    prompt, prompts,
    qdrant::Qdrant,
    record::{html::HTML, Content, Record},
};
use scraper::{Html, Selector};

pub struct RustBookScraper {
    bert: Bert,
    qdrant: Qdrant,
    collection_name: String,
}

impl RustBookScraper {
    pub async fn new(collection_name: String) -> Result<Self> {
        let bert = Bert::new().build_model_and_tokenizer().await?;
        let qdrant = Qdrant::new("http://localhost:6334")?;

        Ok(Self {
            bert,
            qdrant,
            collection_name,
        })
    }

    pub async fn scrape_and_insert(&self) -> Result<()> {
        let pages = self.get_book_pages().await?;
        let mut records = Vec::new();

        for page in pages {
            let html = self.get_page_html(&page).await?;
            let content = self.extract_content(&html);
            records.push(Record::new(Content::String(content)));
        }

        let embeddings = self.bert.generate_embeddings(prompts!(&records)).await?;
        self.qdrant
            .insert_many(&self.collection_name, embeddings.to_vec2()?, records)
            .await?;

        Ok(())
    }

    pub async fn query_and_get_prompt(&self, query: &str) -> Result<String> {
        let query_embedding = self.bert.generate_embedding(prompt!(query)).await?;
        let results = self
            .qdrant
            .search(
                &self.collection_name,
                query_embedding.to_vec()?.clone(),
                3,
                None,
            )
            .await?;

        let prompt_for_model = r#"
        {{#chat}}
            {{#system}}
            You are an expert Rust programmer and teacher. You have been given a question about Rust and some relevant information from the Rust Book. Use this information to provide a comprehensive and accurate answer to the user's question.
            {{/system}}

            {{#user}}
            {{user_query}}
            {{/user}}

            {{#system}}
            Based on the retrieved information from the Rust Book, here are the relevant passages:

            {{#each relevant_info}}
            {{this}}
            {{/each}}

            Please provide a detailed answer to the user's question, integrating insights from these passages and your expert knowledge of Rust.
            {{/system}}
        {{/chat}}
        "#;

        let context = serde_json::json!({
            "user_query": query,
            "relevant_info": results
                .iter()
                .filter_map(|found_point| {
                    found_point.payload.as_ref().map(|payload| {
                        serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string())
                    })
                })
                .collect::<Vec<String>>()
        });

        Ok(prompt_for_model.replace("{{user_query}}", query).replace(
            "{{#each relevant_info}}\n{{this}}\n{{/each}}",
            &context["relevant_info"].to_string(),
        ))
    }

    async fn get_book_pages(&self) -> Result<Vec<String>> {
        let base_url = "https://doc.rust-lang.org/book/";
        let html_content = HTML::from_url(base_url).await?;
        let html = Html::parse_document(&html_content.body);

        let selector = Selector::parse("ol.chapter li a").unwrap();
        let pages = html
            .select(&selector)
            .filter_map(|element| element.value().attr("href"))
            .map(|href| format!("{}{}", base_url, href))
            .collect();

        Ok(pages)
    }

    async fn get_page_html(&self, url: &str) -> Result<Html> {
        let html = HTML::from_url(url).await?;
        Ok(Html::parse_document(&html.body))
    }

    fn extract_content(&self, html: &Html) -> String {
        let main_content_selector = Selector::parse("main").unwrap();
        let main_content = html.select(&main_content_selector).next().unwrap();

        main_content.text().collect::<Vec<_>>().join(" ")
    }
}
