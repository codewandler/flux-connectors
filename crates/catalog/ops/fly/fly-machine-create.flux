op fly-machine-create(app_name: String, image: String) -> Any
  description "Create and launch a Fly Machine from one image using Fly's generated name and region placement; this begins billable compute. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.machines.dev/v1"
  $url = fmt("{base}/apps/{app_name}/machines")
  $content_type = "application/json"
  $payload = { config: { image: $image } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
