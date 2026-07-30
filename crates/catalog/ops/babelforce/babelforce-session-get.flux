op babelforce-session-get(id: String) -> Any
  description "Get the IVR variables for a session"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/sessions/{id}")
  $response = http.request({ method: "GET", url: $url })
  return $response
