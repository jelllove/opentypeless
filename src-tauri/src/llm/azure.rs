/// Azure resource keys use `api-key`, not the generic BYOK bearer header.
pub fn is_provider(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "azure-openai" | "azure-foundry"
    )
}

pub fn endpoint(base: &str, models: bool) -> Result<String, String> {
    let mut url = url::Url::parse(base.trim())
        .map_err(|_| "Enter a valid Azure HTTPS resource endpoint".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Azure endpoint must use HTTPS without embedded credentials or fragment".into(),
        );
    }
    let path = url.path().trim_end_matches('/').to_string();
    if matches!(path.as_str(), "/openai/responses" | "/openai/v1/responses") {
        if path == "/openai/responses" && !has_api_version(&url) {
            return Err("Azure preview Responses URLs require an api-version query".into());
        }
        if models {
            url.set_path("/openai/v1/models");
            url.set_query(None);
        } else {
            url.set_path(&path);
        }
        return Ok(url.into());
    }
    if matches!(path.as_str(), "" | "/openai") && has_api_version(&url) {
        if models {
            url.set_path("/openai/v1/models");
            url.set_query(None);
        } else {
            // The deployment name is supplied by endpoint_for_model.
            url.set_path("/openai");
        }
        return Ok(url.into());
    }
    if path.starts_with("/openai/deployments/") {
        let deployment = path.strip_prefix("/openai/deployments/").unwrap();
        let deployment = deployment
            .strip_suffix("/chat/completions")
            .unwrap_or(deployment);
        if deployment.is_empty() || deployment.contains('/') {
            return Err("Invalid Azure deployment endpoint".into());
        }
        if !url
            .query_pairs()
            .any(|(key, value)| key == "api-version" && !value.is_empty())
        {
            return Err("Legacy Azure deployment URLs require an api-version query; alternatively use the resource /openai/v1/ endpoint".into());
        }
        if models {
            return Err("Enter your Azure deployment name manually; model listing requires the resource /openai/v1/ endpoint".into());
        }
        url.set_path(&format!(
            "/openai/deployments/{deployment}/chat/completions"
        ));
    } else {
        if url.query().is_some() {
            return Err("Azure v1 endpoints do not require query parameters".into());
        }
        let root = match path.as_str() {
            "" | "/openai" | "/openai/v1" | "/openai/v1/chat/completions" | "/openai/v1/models" => "/openai/v1",
            _ => return Err("Use your Azure resource endpoint or /openai/v1/ endpoint, not a Foundry project URL".into()),
        };
        url.set_path(&format!(
            "{root}/{}",
            if models { "models" } else { "chat/completions" }
        ));
    }
    Ok(url.into())
}

fn has_api_version(url: &url::Url) -> bool {
    url.query_pairs()
        .any(|(key, value)| key == "api-version" && !value.trim().is_empty())
}

pub fn uses_responses(base: &str) -> bool {
    url::Url::parse(base.trim()).ok().is_some_and(|url| {
        matches!(
            url.path().trim_end_matches('/'),
            "/openai/responses" | "/openai/v1/responses"
        )
    })
}

pub fn endpoint_for_model(base: &str, model: &str) -> Result<String, String> {
    let endpoint = endpoint(base, false)?;
    let mut url = url::Url::parse(&endpoint).map_err(|error| error.to_string())?;
    if url.path() == "/openai" && has_api_version(&url) {
        if model.trim().is_empty() {
            return Err("Azure deployment name is required".into());
        }
        url.path_segments_mut()
            .map_err(|_| "Invalid Azure URL".to_string())?
            .clear()
            .push("openai")
            .push("deployments")
            .push(model.trim())
            .push("chat")
            .push("completions");
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn azure_responses_preview_and_v1_urls_are_preserved() {
        let preview =
            "https://example.openai.azure.com/openai/responses?api-version=2025-04-01-preview";
        assert_eq!(endpoint(preview, false).unwrap(), preview);
        assert_eq!(
            endpoint("https://test.openai.azure.com/openai/v1/responses/", false).unwrap(),
            "https://test.openai.azure.com/openai/v1/responses"
        );
        assert_eq!(
            endpoint(preview, true).unwrap(),
            "https://example.openai.azure.com/openai/v1/models"
        );
        assert!(endpoint("https://test.openai.azure.com/openai/responses", false).is_err());
    }

    #[test]
    fn azure_versioned_root_matches_sdk_deployment_route() {
        assert_eq!(endpoint_for_model("https://example.openai.azure.com/?api-version=2024-12-01-preview", "gpt-6.1-sol").unwrap(), "https://example.openai.azure.com/openai/deployments/gpt-6.1-sol/chat/completions?api-version=2024-12-01-preview");
        assert!(endpoint_for_model(
            "https://test.openai.azure.com/?api-version=2024-12-01-preview",
            " "
        )
        .is_err());
    }

    #[test]
    fn azure_v1_and_legacy_endpoints_are_not_double_appended() {
        assert_eq!(
            endpoint("https://test.openai.azure.com/openai/v1/", false).unwrap(),
            "https://test.openai.azure.com/openai/v1/chat/completions"
        );
        assert_eq!(
            endpoint(
                "https://test.services.ai.azure.com/openai/v1/chat/completions",
                true
            )
            .unwrap(),
            "https://test.services.ai.azure.com/openai/v1/models"
        );
        let legacy = "https://test.openai.azure.com/openai/deployments/my-gpt/chat/completions?api-version=2024-10-21";
        assert_eq!(endpoint(legacy, false).unwrap(), legacy);
        assert!(endpoint(legacy, true).is_err());
        assert!(endpoint(
            "https://test.openai.azure.com/openai/deployments/my-gpt",
            false
        )
        .is_err());
        for base in [
            "http://example.com",
            "https://key@example.com",
            "https://example.com/#fragment",
            "https://example.com/api/projects/project",
            "https://example.com/?key=secret",
        ] {
            assert!(endpoint(base, false).is_err());
        }
    }
}
