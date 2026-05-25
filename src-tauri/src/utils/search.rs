pub struct SearchQuery {
    pub terms: Vec<String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
}

impl SearchQuery {
    pub fn parse(input: &str) -> Self {
        let mut terms = Vec::new();
        let mut include_tags = Vec::new();
        let mut exclude_tags = Vec::new();

        let mut current_token = String::new();
        let mut in_quotes = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                }
                ' ' if !in_quotes => {
                    if !current_token.is_empty() {
                        Self::process_token(&current_token, &mut terms, &mut include_tags, &mut exclude_tags);
                        current_token.clear();
                    }
                }
                _ => {
                    current_token.push(ch);
                }
            }
        }

        if !current_token.is_empty() {
            Self::process_token(&current_token, &mut terms, &mut include_tags, &mut exclude_tags);
        }

        Self {
            terms,
            include_tags,
            exclude_tags,
        }
    }

    fn process_token(
        token: &str,
        terms: &mut Vec<String>,
        include_tags: &mut Vec<String>,
        exclude_tags: &mut Vec<String>,
    ) {
        if token.starts_with("tag:") {
            let tag = token[4..].to_string();
            include_tags.push(tag);
        } else if token.starts_with("-tag:") {
            let tag = token[5..].to_string();
            exclude_tags.push(tag);
        } else if token.starts_with('-') {
            exclude_tags.push(token[1..].to_string());
        } else {
            terms.push(token.to_string());
        }
    }

    pub fn to_sql_conditions(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // Text search terms
        for term in &self.terms {
            conditions.push("a.title LIKE ?".to_string());
            params.push(format!("%{}%", term));
        }

        // Include tags (INNER JOIN)
        if !self.include_tags.is_empty() {
            for tag in &self.include_tags {
                if let Some((namespace, name)) = tag.split_once(':') {
                    conditions.push(
                        "a.id IN (SELECT at.archive_id FROM archive_tags at JOIN tags t ON t.id = at.tag_id WHERE t.namespace = ? AND t.name = ?)".to_string()
                    );
                    params.push(namespace.to_string());
                    params.push(name.to_string());
                } else {
                    conditions.push(
                        "a.id IN (SELECT at.archive_id FROM archive_tags at JOIN tags t ON t.id = at.tag_id WHERE t.name = ?)".to_string()
                    );
                    params.push(tag.to_string());
                }
            }
        }

        // Exclude tags (NOT IN)
        if !self.exclude_tags.is_empty() {
            for tag in &self.exclude_tags {
                if let Some((namespace, name)) = tag.split_once(':') {
                    conditions.push(
                        "a.id NOT IN (SELECT at.archive_id FROM archive_tags at JOIN tags t ON t.id = at.tag_id WHERE t.namespace = ? AND t.name = ?)".to_string()
                    );
                    params.push(namespace.to_string());
                    params.push(name.to_string());
                } else {
                    conditions.push(
                        "a.id NOT IN (SELECT at.archive_id FROM archive_tags at JOIN tags t ON t.id = at.tag_id WHERE t.name = ?)".to_string()
                    );
                    params.push(tag.to_string());
                }
            }
        }

        let sql = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        (sql, params)
    }
}
