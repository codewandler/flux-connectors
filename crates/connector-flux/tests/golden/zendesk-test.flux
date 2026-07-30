op zendesk-test -> Any
  description "Verify Zendesk credentials by fetching the authenticated user."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://example.zendesk.com"
  $url = fmt("{base}/api/v2/users/me.json")
  $response = http.request({ method: "GET", url: $url })
  return $response
