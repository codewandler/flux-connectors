op freshdesk-contact-get(id: String) -> Any
  description "Get one contact"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{domain}/api/v2"
  $url = fmt("{base}/contacts/{id}")
  $response = http.request({ method: "GET", url: $url })
  return $response
