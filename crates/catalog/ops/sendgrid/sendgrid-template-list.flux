op sendgrid-template-list(generations: String, page_size: Number) -> Any
  description "List transactional templates (name, id, generation, and each template's versions). Also this connector's `verify` — a bounded read that runs unattended"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/templates")
  response = http.request(method: "GET", query: { generations, page_size }, url)
  return response
