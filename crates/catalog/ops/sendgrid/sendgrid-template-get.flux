op sendgrid-template-get(template_id: String) -> Any
  description "Get one transactional template by id, including every version's subject and content"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/templates/{template_id}")
  response = http.request(method: "GET", url)
  return response
