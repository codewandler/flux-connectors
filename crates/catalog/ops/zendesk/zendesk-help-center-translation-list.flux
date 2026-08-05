op zendesk-help-center-translation-list(article_id: Number) -> Any
  description "List every translation of one Help Center article without exposing unencoded locale filters"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/articles/{article_id}/translations")
  response = http.request(method: "GET", url)
  return response
