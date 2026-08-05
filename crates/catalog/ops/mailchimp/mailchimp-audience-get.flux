op mailchimp-audience-get(list_id: String) -> Any
  description "Get one audience by id, with its settings and current member counts"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/lists/{list_id}")
  response = http.request(method: "GET", url)
  return response
