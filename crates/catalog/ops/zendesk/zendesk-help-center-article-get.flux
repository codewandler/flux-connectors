op zendesk-help-center-article-get(article_id: Number) -> Any
  description "Get one Help Center article by its numeric id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/articles/{article_id}")
  response = http.request(method: "GET", url)
  return response
