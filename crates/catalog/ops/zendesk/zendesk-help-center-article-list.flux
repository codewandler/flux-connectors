op zendesk-help-center-article-list(start_time: Number) -> Any
  description "List Help Center articles, optionally limited to articles updated since a Unix timestamp"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/articles")
  sep = "?"
  when start_time
    url = fmt("{url}{sep}start_time={start_time}")
  response = http.request(method: "GET", url)
  return response
