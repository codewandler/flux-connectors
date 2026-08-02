op zendesk-help-center-article-incremental-list(start_time: Number) -> Any
  description "Incrementally list Help Center articles updated since an optional integer Unix timestamp"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/incremental/articles")
  sep = "?"
  when start_time
    url = fmt("{url}{sep}start_time={start_time}")
  response = http.request(method: "GET", url)
  return response
