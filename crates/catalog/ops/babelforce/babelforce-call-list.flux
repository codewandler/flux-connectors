op babelforce-call-list(page: Number, max: Number, id: Any, sessionId: String, conversationId: String, agentId: Any, fromNumber: Any, toNumber: Any, type: String, state: String, finishReason: String, time_start: Number, time_end: Number, q: String) -> Any
  description "List and filter calls from the reporting view"
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
  when $id
    $url = fmt("{url}{sep}id={id}")
    $sep = "&"
  when $sessionId
    $url = fmt("{url}{sep}sessionId={sessionId}")
    $sep = "&"
  when $conversationId
    $url = fmt("{url}{sep}conversationId={conversationId}")
    $sep = "&"
  when $agentId
    $url = fmt("{url}{sep}agentId={agentId}")
    $sep = "&"
  when $fromNumber
    $url = fmt("{url}{sep}fromNumber={fromNumber}")
    $sep = "&"
  when $toNumber
    $url = fmt("{url}{sep}toNumber={toNumber}")
    $sep = "&"
  when $type
    $url = fmt("{url}{sep}type={type}")
    $sep = "&"
  when $state
    $url = fmt("{url}{sep}state={state}")
    $sep = "&"
  when $finishReason
    $url = fmt("{url}{sep}finishReason={finishReason}")
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
