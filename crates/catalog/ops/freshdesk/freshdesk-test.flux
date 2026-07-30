op freshdesk-test(per_page: Number) -> Any
  description "Verify credentials with a bounded contact read"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{domain}/api/v2"
  $url = fmt("{base}/contacts")
  $sep = "?"
  when $per_page
    $url = fmt("{url}{sep}per_page={per_page}")
  $response = http.request({ method: "GET", url: $url })
  return $response
