op openrouter-credits-get -> Any
  description "Read the account's total purchased credits and total usage, in credit units"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://openrouter.ai"
  $url = fmt("{base}/api/v1/credits")
  $response = http.request({ method: "GET", url: $url })
  return $response
