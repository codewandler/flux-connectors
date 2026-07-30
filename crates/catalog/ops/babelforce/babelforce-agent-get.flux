op babelforce-agent-get(id: String) -> Any
  description "Get one agent"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/agents/{id}")
  $response = http.request({ method: "GET", url: $url })
  return $response
