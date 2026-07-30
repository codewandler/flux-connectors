op openai-models-list -> Any
  description "List the models available to this API key, with their ids and owners"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://api.openai.com"
  $url = fmt("{base}/v1/models")
  $response = http.request({ method: "GET", url: $url })
  return $response
