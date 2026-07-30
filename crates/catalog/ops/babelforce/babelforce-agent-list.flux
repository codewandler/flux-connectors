op babelforce-agent-list(page: Number, max: Number, q: String, enabled: Bool, name: String, number: String, sourceId: String, source: String, state: String, groupIds: Any, groups: Any) -> Any
  description "List and filter agents. Doubles as the verification operation — cheap, read-only, and it fails loudly on a bad credential; this API has no /me endpoint"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/agents")
  $sep = "?"
  when $page
    $url = fmt("{url}{sep}page={page}")
    $sep = "&"
  when $max
    $url = fmt("{url}{sep}max={max}")
    $sep = "&"
  when $q
    $url = fmt("{url}{sep}q={q}")
    $sep = "&"
  when $enabled
    $url = fmt("{url}{sep}enabled={enabled}")
    $sep = "&"
  when $name
    $url = fmt("{url}{sep}name={name}")
    $sep = "&"
  when $number
    $url = fmt("{url}{sep}number={number}")
    $sep = "&"
  when $sourceId
    $url = fmt("{url}{sep}sourceId={sourceId}")
    $sep = "&"
  when $source
    $url = fmt("{url}{sep}source={source}")
    $sep = "&"
  when $state
    $url = fmt("{url}{sep}state={state}")
    $sep = "&"
  when $groupIds
    $url = fmt("{url}{sep}groupIds={groupIds}")
    $sep = "&"
  when $groups
    $url = fmt("{url}{sep}groups={groups}")
  $response = http.request({ method: "GET", url: $url })
  return $response
