op zendesk-help-center-article-create(section_id: Number, article: Any) -> Any
  description "Publish a new externally visible Help Center article in one numeric section"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/sections/{section_id}/articles")
  content_type = "application/json"
  payload = { article }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
