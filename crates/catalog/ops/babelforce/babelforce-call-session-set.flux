op babelforce-call-session-set(id: String, body: Any) -> Any
  description "Set session variables on a live call. Variable keys must start with `app` — babelforce rejects other keys, and states the rule only in prose"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/calls/{id}/session/set")
  $content_type = "application/json"
  $payload = parse($body, as: "json")
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
