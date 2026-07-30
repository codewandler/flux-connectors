op freshdesk-ticket-get(id: String) -> Any
  description "View one ticket"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{domain}/api/v2"
  $url = fmt("{base}/tickets/{id}")
  $response = http.request({ method: "GET", url: $url })
  return $response
