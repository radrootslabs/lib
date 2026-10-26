import init, {
  BindingFixture, Client, Workspace, CommandRequest, ListingInput, InquiryInput,
  Receipt, Failure, Admission, BusinessInstallation, Issuance, OperationKind,
  SessionState, StartMode, TargetObservation, verify_listing_publications,
  verify_binding_inquiry, advertisements,
  FixtureRejection, SignerRejectionReason,
} from '@radroots/browser-feasibility';

function assert(value: unknown, message: string): asserts value {
  if (!value) throw Error(message);
}
const decode = (value: Uint8Array): unknown => JSON.parse(new TextDecoder().decode(value));
const id = (byte: string): string => byte.repeat(32);
const peer = 'c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5';
const now = (): string => String(Math.floor(Date.now() / 1000));
const deadline = (): string => String(Math.floor(Date.now() / 1000) + 120);
const text = '\uFEFF  cafe\u0301 / café 🥕\u0000  ';

async function rejectedSigners(base: string, context: string): Promise<void> {
  const cases = [
    {mode:FixtureRejection.Malformed,reason:SignerRejectionReason.InvalidArtifact,covered:'34',total:'34',digest:'ba883ea08dff4c078672a055dd2be042d9debb5b22c66527e4499c068011879d'},
    {mode:FixtureRejection.Oversized,reason:SignerRejectionReason.TooLarge,covered:'65536',total:'65537',digest:'d2d5af5c0316f5c2b119635386239edc87abce035f851c3f6ee127e6c3cbf435'},
    {mode:FixtureRejection.Foreign,reason:SignerRejectionReason.BindingMismatch,covered:undefined,total:undefined,digest:undefined},
  ];
  for (const value of cases) {
    const host=await BindingFixture.create_rejected(base.replace('http:','ws:')+'/listing/'+context,`rejected_${context}_${value.mode}`,value.mode);
    const client=host.client(),scope=host.workspace(); await client.start(StartMode.Online);
    const request=new CommandRequest(id('18'),deadline()),input=listingInput();
    let refused=false;
    try { await client.publish_listing(scope,request,input); }
    catch(error:unknown) {
      assert(error instanceof Failure,'actual generated rejected failure'); refused=true;
      assert(error.code==='signer_tampering'&&error.retry==='manual_reconciliation','original failure code/retry');
      assert(error.operation_id===id('18')&&error.admission===Admission.Confirmed,'original confirmed identity');
      const diagnostic=error.signer_rejection; assert(diagnostic,'bounded signer metadata');
      assert(diagnostic.reason===value.reason,'original fixed rejection reason');
      const fingerprint=diagnostic.fingerprint;
      if(fingerprint){
        assert(fingerprint.covered_bytes===value.covered&&fingerprint.total_bytes===value.total,'exact coverage strings');
        const bytes=fingerprint.sha256;
        assert(bytes.length===32&&Array.from(bytes).map(byte=>byte.toString(16).padStart(2,'0')).join('')===value.digest,'independent original prefix digest');
        fingerprint.free();
      }else assert(value.digest===undefined,'no invented or discarded fingerprint');
      diagnostic.free(); error.free();
    }
    assert(refused&&host.signing_calls()===1,'one actual callback and no successful signature');
    const facts=await client.operation(scope,id('18'),undefined);
    assert(facts&&facts.business_installation===BusinessInstallation.NotInstalled,'rejection is no business success');
    facts.free(); await client.close(); assert(host.callbacks_closed()===1,'rejected callback owner closes'); scope.close();
    for(const item of [request,input,scope,client,host])item.free();
  }
}

async function refuses(call: () => unknown, code: string): Promise<void> {
  try { await call(); } catch (error: unknown) {
    assert(error instanceof Failure, 'typed generated failure');
    assert(error.code === code, `safe failure ${error.code} expected ${code}`);
    assert(!JSON.stringify([error.code,error.field,error.retry]).includes(text), 'input-free failure');
    error.free(); return;
  }
  throw Error(`expected ${code}`);
}
function receipt(value: Receipt, operation: string, kind: OperationKind): void {
  assert(value.operation_id === operation && value.kind === kind, 'exact typed operation/family');
  assert(value.admission === Admission.Confirmed, 'confirmed durable admission');
  assert(value.business_installation === BusinessInstallation.Installed, 'actual installation');
  assert(value.issuance === Issuance.Obtained, 'actual obtained originals');
  for (const digest of [value.command_digest,value.operation_checkpoint,value.admission_transaction_id,
    value.plan_transaction_id,value.installation_transaction_id]) assert(digest && /^[0-9a-f]{64}$/.test(digest), 'exact digest/transaction');
  assert(typeof value.prepared_at === 'string' && typeof value.first_issue_not_after === 'string'
    && typeof value.installed_at === 'string' && typeof value.observed_at === 'string', 'exact time strings');
  assert(decode(value.captured_context_json) && decode(value.authority_checkpoint_json), 'source-owned structured context/checkpoint');
  assert(value.missing_sources.length === 0 && value.conflicting_records.length === 0
    && value.conflicting_sources.length === 0 && value.error === undefined, 'all conflict/error axes');
  assert(value.record_ids.length === 0, 'known listing/chat have no fabricated structured-commerce record IDs');
  const artifacts=value.artifacts;
  assert(artifacts.length > 0, 'actual original artifacts');
  for (const artifact of artifacts) {
    assert(artifact.issuance === Issuance.Obtained && artifact.attempt_id && artifact.claim_transaction
      && artifact.intent_digest && artifact.capture_transaction && artifact.obtained_digest
      && artifact.publication_transaction, 'complete original artifact provenance');
    assert(artifact.observed_at_unix_ms === undefined || typeof artifact.observed_at_unix_ms === 'string', 'nullable exact artifact time');
    artifact.free();
  }
  for (const attempt of value.delivery_attempts) {
    const policy = attempt.policy;
    assert(policy.kind === 'all' && policy.threshold === undefined && policy.fingerprints.length === 0, 'original closed policy');
    const targets=attempt.targets;
    assert(attempt.selected_fingerprints.length === 1 && targets.length === 1
      && attempt.target_ordinals.length === 1, 'actual target selection');
    for (const target of targets) {
      assert(target.uri.startsWith('ws://127.0.0.1:') && target.kind && target.fingerprint, 'actual configured target');
      target.free();
    }
    for (const field of [attempt.request_id,attempt.event_id,attempt.publication_policy_digest,attempt.intent_digest,
      attempt.original_bytes_digest,attempt.transaction_id,attempt.work_transaction_id,attempt.retained_at,attempt.deadline_unix_ms]) assert(typeof field === 'string' && field.length > 0, 'complete original delivery commitment');
    policy.free(); attempt.free();
  }
  for (const observation of value.delivery_observations) {
    assert(observation.event_id && observation.target && observation.request_id && observation.target_fingerprint
      && observation.target_ordinal > 0, 'complete correlated observation');
    assert(observation.observed_at_unix_ms === undefined || typeof observation.observed_at_unix_ms === 'string', 'nullable exact observation time');
    observation.free();
  }
  assert(typeof value.delivery === 'number' && typeof value.wait_termination === 'number'
    && Array.isArray(value.recovery), 'independent delivery/wait/recovery axes');
}
function listingInput(): ListingInput {
  return ListingInput.parse(new TextEncoder().encode(JSON.stringify({
    id:'01'.repeat(16),title:'  Packed carrots 🥕  ',summary:'Quote required',description_markdown:'Exact public terms',published_at:now(),
    profile:{profile:'radroots.listing',version:1,state:'active',product:{name:'Carrots',description:'Fresh carrots',producer:null,origin_summary:null},
      default_option:id('02'),options:[{id:id('02'),label:'Bunch',unit:{type:'package',label:'bunch',contents:{type:'unspecified'}},
        increment:'1',minimum:'1',price:{type:'quote_required'},supply:{type:'available',amount:'10'},lead_seconds:'0'}],
      fulfillment:{modes:['pickup'],area:'Example',lead_seconds:'0'},authority:null,media:[]},
  })));
}
async function query(client: Client, scope: Workspace, operation: string, digest: string): Promise<Receipt> {
  const result = await client.operation(scope,operation,digest);
  assert(result, 'actual retained original operation'); return result;
}
export async function run(base: string, context: string): Promise<unknown> {
  await init();
  const publicRoute = '/listing/'+context, privateRoute = '/'+context;
  await fetch(base+'/advertisements'+privateRoute,{method:'POST',body:advertisements(base.replace('http:','ws:')+privateRoute)});
  const seller = await BindingFixture.create(base.replace('http:','ws:')+publicRoute,'packed_public_'+context,true);
  const client = seller.client(), scope = seller.workspace();
  assert(client.state() === SessionState.Ready, 'inert configured host');
  await client.start(StartMode.Online);
  await refuses(()=>new CommandRequest(id('11').toUpperCase(),'253402300800'), 'InvalidInput');
  await refuses(()=>new CommandRequest(id('ab').toUpperCase(),deadline()), 'InvalidInput');
  await refuses(()=>new CommandRequest(id('11'),'0001'), 'InvalidInput');
  await refuses(()=>new InquiryInput(id('12'),peer,undefined,'x'.repeat(17000)), 'LimitExceeded');
  await refuses(()=>new InquiryInput(id('12'),peer,undefined,'\ud800'), 'InvalidInput');
  await refuses(()=>ListingInput.parse(new Uint8Array(65537)), 'LimitExceeded');
  await refuses(()=>ListingInput.parse(new TextEncoder().encode('{"id":"bad"}')), 'InvalidInput');
  const input=listingInput(), request=new CommandRequest(id('13'),deadline(),'packed-public');
  const original=await client.publish_listing(scope,request,input);
  receipt(original,id('13'),OperationKind.Listing);
  assert(original.message_id === undefined, 'public has no private message');
  const result=original.business_result; assert(result, 'typed public value');
  assert(result.chat_value() === undefined, 'public has no private placeholder');
  const listing=result.listing_value(); assert(listing && listing.title() === '  Packed carrots 🥕  '
    && listing.id() === '01'.repeat(16) && listing.published_at() && decode(listing.terms_json()), 'actual listing content');
  const revision=listing.revision(); listing.free(); result.free();
  for (const artifact of original.artifacts) { assert(artifact.recipient === undefined, 'public recipient absent'); artifact.free(); }
  const observations=original.delivery_observations;
  assert(observations.length === 1 && observations[0].outcome === TargetObservation.Rejected, 'actual public rejection');
  observations.forEach(value=>value.free());
  const digest=original.command_digest, checkpoint=original.operation_checkpoint;
  const replay=await client.publish_listing(scope,request,input);
  assert(replay.operation_checkpoint === checkpoint, 'idempotent JS replay'); replay.free();
  await client.close(); await client.close(); assert(seller.callbacks_closed() === 1, 'original callbacks closed once');
  const reopened=await seller.reopen(), resumedClient=reopened.client(), resumedScope=reopened.workspace();
  await resumedClient.start(StartMode.Online);
  const retained=await query(resumedClient,resumedScope,id('13'),digest);
  assert(retained.operation_checkpoint === checkpoint, 'reopen exact original checkpoint'); retained.free();
  const resumed=await resumedClient.resume(resumedScope,id('13'),digest);
  receipt(resumed,id('13'),OperationKind.Listing);
  assert(resumed.installation_transaction_id === original.installation_transaction_id
    && resumed.first_issue_not_after === original.first_issue_not_after, 'original issuance/installation after retry');
  const attempts=resumed.delivery_attempts;
  assert(attempts.length === 2 && attempts[0].original_bytes_digest === attempts[1].original_bytes_digest
    && attempts[0].publication_policy_digest === attempts[1].publication_policy_digest, 'exact public retry/policy');
  assert(attempts.flatMap(value=>Array.from(value.target_ordinals)).sort().join(',') === '1,2', 'per-target original ordinals');
  attempts.forEach(value=>value.free());
  const accepted=resumed.delivery_observations;
  assert(accepted.some(value=>value.outcome === TargetObservation.Accepted), 'actual retry acceptance'); accepted.forEach(value=>value.free());
  const publicEvents=await fetch(base+'/events'+publicRoute).then(response=>response.text());
  verify_listing_publications(publicEvents,revision);
  const buyer=await BindingFixture.create(base.replace('http:','ws:')+privateRoute,'packed_private_'+context,false);
  const buyerClient=buyer.client(), buyerScope=buyer.workspace(); await buyerClient.start(StartMode.Online);
  const denied=new CommandRequest(id('14'),deadline());
  await refuses(()=>buyerClient.publish_listing(buyerScope,denied,input),'unauthorized');
  await refuses(()=>resumedClient.publish_listing(buyerScope,denied,input),'unauthorized');
  const inquiry=new InquiryInput(id('15'),peer,undefined,text), privateRequest=new CommandRequest(id('16'),deadline());
  const message=await buyerClient.inquire(buyerScope,privateRequest,inquiry);
  receipt(message,id('16'),OperationKind.Chat);
  const business=message.business_result; assert(business && business.listing_value() === undefined, 'private typed family');
  const chat=business.chat_value(); assert(chat && chat.text() === text && chat.thread_id() === id('15') && chat.peer() === peer
    && chat.relation() === 'inquire' && chat.source() === undefined && chat.parent() === undefined, 'exact private original text/relation');
  const messageId=chat.id(); chat.free(); business.free();
  await verify_binding_inquiry(await fetch(base+'/events'+privateRoute).then(response=>response.text()),messageId,text);
  const privateDigest=message.command_digest, privateCheckpoint=message.operation_checkpoint;
  await buyerClient.close(); assert(buyer.callbacks_closed() === 1, 'private callbacks close once');
  const privateReopened=await buyer.reopen(), readClient=privateReopened.client(), readScope=privateReopened.workspace();
  await readClient.start(StartMode.LocalOnly);
  const read=await query(readClient,readScope,id('16'),privateDigest);
  assert(read.operation_checkpoint === privateCheckpoint && read.message_id === messageId, 'private exact reopen without network');
  await readClient.close(); await resumedClient.close();
  assert(privateReopened.callbacks_closed() === 1 && reopened.callbacks_closed() === 1, 'all original callback scopes close');
  await refuses(()=>client.start(StartMode.Online),'capability_unavailable');
  await rejectedSigners(base,context);
  scope.close(); resumedScope.close(); buyerScope.close(); readScope.close();
  for (const value of [original,resumed,message,read]) value.free();
  for (const value of [request,denied,privateRequest,input,inquiry,scope,resumedScope,buyerScope,readScope,client,resumedClient,buyerClient,readClient,seller,reopened,buyer,privateReopened]) value.free();
  return {passed:['packed generated public call and exact replay','reopen/query and original exact-byte retry','all typed receipt axes','private JS call and independent nested authentication','bounded inputs, scope and lifecycle refusals','actual ordered close','bounded rejected signer metadata and exact coverage'],revision,messageId};
}
