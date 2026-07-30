op fly-regions-list -> Any
  description "List Fly's available regions and identify the nearest one; also verifies the configured access token. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://api.machines.dev/v1"
  $url = fmt("{base}/platform/regions")
  $response = http.request({ method: "GET", url: $url })
  return $response
