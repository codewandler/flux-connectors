op babelforce-call-list(page: Number, max: Number, sessionId: String, conversationId: String, id: Any, parentId: Any, type: Any, from: String, fromNumber: String, to: Any, toNumber: Any, time_start: Number, time_end: Number, agentId: Any, q: String, state: Any, domain: Any, source: Any, finishReason: Any, anonymous: Bool, filters_sessionId: String, filters_conversationId: String, filters_id: Any, filters_parentId: Any, filters_type: Any, filters_from: String, filters_fromNumber: String, filters_to: Any, filters_toNumber: Any, filters_time_start: Number, filters_time_end: Number, filters_agentId: Any, filters_q: String, filters_state: Any, filters_domain: Any, filters_source: Any, filters_finishReason: Any, filters_anonymous: Bool) -> Any
  description "List and filter calls from the reporting view. Prefer the flat parameters: every filter is also declared under a `filters.` prefix with an identical schema, and the two are synonyms. Prefer `from`/`to` over `fromNumber`/`toNumber`, which the vendor documents as aliases of them"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when sessionId
    url = fmt("{url}{sep}sessionId={sessionId}")
    sep = "&"
  when conversationId
    url = fmt("{url}{sep}conversationId={conversationId}")
    sep = "&"
  when id
    url = fmt("{url}{sep}id={id}")
    sep = "&"
  when parentId
    url = fmt("{url}{sep}parentId={parentId}")
    sep = "&"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when from
    url = fmt("{url}{sep}from={from}")
    sep = "&"
  when fromNumber
    url = fmt("{url}{sep}fromNumber={fromNumber}")
    sep = "&"
  when to
    url = fmt("{url}{sep}to={to}")
    sep = "&"
  when toNumber
    url = fmt("{url}{sep}toNumber={toNumber}")
    sep = "&"
  when time_start
    url = fmt("{url}{sep}time.start={time_start}")
    sep = "&"
  when time_end
    url = fmt("{url}{sep}time.end={time_end}")
    sep = "&"
  when agentId
    url = fmt("{url}{sep}agentId={agentId}")
    sep = "&"
  when q
    url = fmt("{url}{sep}q={q}")
    sep = "&"
  when state
    url = fmt("{url}{sep}state={state}")
    sep = "&"
  when domain
    url = fmt("{url}{sep}domain={domain}")
    sep = "&"
  when source
    url = fmt("{url}{sep}source={source}")
    sep = "&"
  when finishReason
    url = fmt("{url}{sep}finishReason={finishReason}")
    sep = "&"
  when anonymous
    url = fmt("{url}{sep}anonymous={anonymous}")
    sep = "&"
  when filters_sessionId
    url = fmt("{url}{sep}filters.sessionId={filters_sessionId}")
    sep = "&"
  when filters_conversationId
    url = fmt("{url}{sep}filters.conversationId={filters_conversationId}")
    sep = "&"
  when filters_id
    url = fmt("{url}{sep}filters.id={filters_id}")
    sep = "&"
  when filters_parentId
    url = fmt("{url}{sep}filters.parentId={filters_parentId}")
    sep = "&"
  when filters_type
    url = fmt("{url}{sep}filters.type={filters_type}")
    sep = "&"
  when filters_from
    url = fmt("{url}{sep}filters.from={filters_from}")
    sep = "&"
  when filters_fromNumber
    url = fmt("{url}{sep}filters.fromNumber={filters_fromNumber}")
    sep = "&"
  when filters_to
    url = fmt("{url}{sep}filters.to={filters_to}")
    sep = "&"
  when filters_toNumber
    url = fmt("{url}{sep}filters.toNumber={filters_toNumber}")
    sep = "&"
  when filters_time_start
    url = fmt("{url}{sep}filters.time.start={filters_time_start}")
    sep = "&"
  when filters_time_end
    url = fmt("{url}{sep}filters.time.end={filters_time_end}")
    sep = "&"
  when filters_agentId
    url = fmt("{url}{sep}filters.agentId={filters_agentId}")
    sep = "&"
  when filters_q
    url = fmt("{url}{sep}filters.q={filters_q}")
    sep = "&"
  when filters_state
    url = fmt("{url}{sep}filters.state={filters_state}")
    sep = "&"
  when filters_domain
    url = fmt("{url}{sep}filters.domain={filters_domain}")
    sep = "&"
  when filters_source
    url = fmt("{url}{sep}filters.source={filters_source}")
    sep = "&"
  when filters_finishReason
    url = fmt("{url}{sep}filters.finishReason={filters_finishReason}")
    sep = "&"
  when filters_anonymous
    url = fmt("{url}{sep}filters.anonymous={filters_anonymous}")
  response = http.request(method: "GET", url)
  return response
