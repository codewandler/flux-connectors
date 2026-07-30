op babelforce-call-list(page: Number, max: Number, agentId: Any, time_start: Number, time_end: Number, q: String) -> Any
  description "List and filter calls, in the reporting view."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/calls/reporting")
  $sep = "?"
  when $page
    $url = fmt("{url}{sep}page={page}")
    $sep = "&"
  when $max
    $url = fmt("{url}{sep}max={max}")
    $sep = "&"
  when $agentId
    $url = fmt("{url}{sep}agentId={agentId}")
    $sep = "&"
  when $time_start
    $url = fmt("{url}{sep}time.start={time_start}")
    $sep = "&"
  when $time_end
    $url = fmt("{url}{sep}time.end={time_end}")
    $sep = "&"
  when $q
    $url = fmt("{url}{sep}q={q}")
  $response = http.request({ method: "GET", url: $url })
  return $response
