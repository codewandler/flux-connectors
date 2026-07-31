op sendgrid-template-list(generations: String, page_size: Number) -> Any
  description "List transactional templates (name, id, generation, and each template's versions). Also this connector's `verify` — a bounded read that runs unattended"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/templates")
  sep = "?"
  when generations
    url = fmt("{url}{sep}generations={generations}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}page_size={page_size}")
  response = http.request(method: "GET", url)
  return response
