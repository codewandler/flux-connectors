op zendesk-ticket-show(ticket_id: Number) -> Any
  description "Show one ticket. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/error` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://example.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  $response = http.request({ method: "GET", url: $url })
  return $response
