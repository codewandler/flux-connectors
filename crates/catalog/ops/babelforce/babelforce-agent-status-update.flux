op babelforce-agent-status-update(id: String, enabled: Bool, presence_name: String) -> Any
  description "Update an agent's status. Supply at least one of enabled or presence.name — the request body's properties are all optional, so an empty PUT is schema-valid and does nothing"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/agents/{id}/status")
  $content_type = "application/json"
  $payload = { enabled: $enabled, presence: { name: $presence_name } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
