op babelforce-session-update(id: String, body: Any) -> Any
  description "Update the user-scoped variables of a session"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/sessions/{id}")
  $content_type = "application/json"
  $payload = parse($body, as: "json")
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
