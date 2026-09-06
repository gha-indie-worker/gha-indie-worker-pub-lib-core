import 'dart:convert';

import 'package:gha_indie_worker_pub_lib_core/gha_indie_worker_pub_lib_core.dart';
import 'package:test/test.dart';

String? Function(String) lookupFrom(Map<String, String> pairs) =>
    (key) => pairs[key];

const String sha = '0123456789abcdef0123456789abcdef01234567';
const int now = 1780000000;

Map<String, Object?> runJson([Map<String, Object?> overrides = const {}]) => {
      'id': 'r1',
      'repository': 'gha-indie-worker/x',
      'revision': sha,
      'workflowPath': '.github/workflows/ci.yml',
      'lane': 'independent',
      'state': 'running',
      'queuedAt': '2026-09-05T12:00:00Z',
      ...overrides,
    };

void main() {
  group('config', () {
    test('a minimal configuration derives the websocket origin', () {
      final config = RuntimeConfig.fromLookup(
        lookupFrom({apiBaseKey: 'https://api.indiebuild.dev'}),
      );
      expect(config.wsBase, 'wss://api.indiebuild.dev');
      expect(config.surface, Surface.app);
      expect(config.timeoutMs, defaultTimeoutMs);
      expect(config.sharedAuthBase, isNull);
    });

    test('a plaintext base derives a plaintext socket and never upgrades', () {
      final config = RuntimeConfig.fromLookup(
        lookupFrom({apiBaseKey: 'http://127.0.0.1:8080'}),
      );
      expect(config.wsBase, 'ws://127.0.0.1:8080');
    });

    test('the api base is required, and whitespace is not presence', () {
      expect(
        () => RuntimeConfig.fromLookup(lookupFrom({})),
        throwsA(isA<MissingConfig>()),
      );
      expect(
        () => RuntimeConfig.fromLookup(lookupFrom({apiBaseKey: '   '})),
        throwsA(isA<MissingConfig>()),
      );
    });

    test('a base without a scheme or carrying a query is rejected', () {
      for (final bad in <String>[
        'api.indiebuild.dev',
        'ftp://api.indiebuild.dev',
        'https://api.indiebuild.dev?tenant=1',
        'https://api.indiebuild.dev#x',
      ]) {
        expect(
          () => RuntimeConfig.fromLookup(lookupFrom({apiBaseKey: bad})),
          throwsA(isA<InvalidConfig>()),
          reason: bad,
        );
      }
    });

    test('every surface round trips and only org shows seat ui', () {
      for (final surface in Surface.values) {
        expect(Surface.parse(surface.wireName), surface);
      }
      expect(Surface.parse('admin'), isNull);
      expect(
        Surface.values.where((s) => s.isOrganization).toList(),
        <Surface>[Surface.org],
      );
    });

    test('a zero, absurd or non-integer timeout is rejected', () {
      for (final bad in <String>['0', '600001', 'not-a-number', '-5']) {
        expect(
          () => RuntimeConfig.fromLookup(lookupFrom({
                apiBaseKey: 'https://api.indiebuild.dev',
                clientTimeoutMsKey: bad,
              })),
          throwsA(isA<InvalidConfig>()),
          reason: bad,
        );
      }
    });

    test('endpoint joins without doubling the separator', () {
      final config = RuntimeConfig.fromLookup(
        lookupFrom({apiBaseKey: 'https://api.indiebuild.dev/'}),
      );
      expect(config.endpoint('/v1/runs'), 'https://api.indiebuild.dev/v1/runs');
      expect(config.endpoint('v1/runs'), 'https://api.indiebuild.dev/v1/runs');
    });

    test('the declared key list is complete and unique', () {
      expect(configKeys.length, 6);
      expect(configKeys.toSet().length, configKeys.length);
    });
  });

  group('pagination', () {
    test('base64url matches the vectors the other cores assert', () {
      expect(base64UrlEncodeUnpadded(utf8.encode('foobar')), 'Zm9vYmFy');
      expect(base64UrlEncodeUnpadded(utf8.encode('f')), 'Zg');
      expect(base64UrlEncodeUnpadded(utf8.encode('fo')), 'Zm8');
    });

    test('base64url round trips and rejects padding or foreign characters', () {
      for (final text in <String>['f', 'fo', 'foo', 'foob', 'fooba', 'foobar']) {
        final encoded = base64UrlEncodeUnpadded(utf8.encode(text));
        expect(base64UrlDecodeStrict(encoded), utf8.encode(text), reason: text);
      }
      for (final bad in <String>['Zm9v=', 'Zm 9v', 'Zm+v', 'Zm/v', '']) {
        expect(
          () => base64UrlDecodeStrict(bad),
          throwsA(isA<InvalidCursor>()),
          reason: bad,
        );
      }
    });

    test('a cursor round trips through its wire form', () {
      final cursor = Cursor('2026-09-05T12-00-00Z', 'run-42');
      expect(Cursor.decode(cursor.encode()), cursor);
    });

    test('a cursor refuses fields that would break its own encoding', () {
      expect(() => Cursor('a:b', 'id'), throwsA(isA<InvalidCursor>()));
      expect(() => Cursor('a', 'i:d'), throwsA(isA<InvalidCursor>()));
      expect(() => Cursor('', 'id'), throwsA(isA<InvalidCursor>()));
    });

    test('a hand-written cursor does not decode', () {
      expect(() => Cursor.decode('not-base64!!'), throwsA(isA<InvalidCursor>()));
      expect(
        () => Cursor.decode(base64UrlEncodeUnpadded(utf8.encode('v2:key:id'))),
        throwsA(isA<InvalidCursor>()),
      );
    });

    test('page requests are bounded at both ends', () {
      expect(() => PageRequest(limit: 0), throwsA(isA<PageLimitOutOfRange>()));
      expect(
        () => PageRequest(limit: maxPageLimit + 1),
        throwsA(isA<PageLimitOutOfRange>()),
      );
      expect(PageRequest(limit: 1).limit, 1);
      expect(PageRequest().limit, defaultPageLimit);
    });

    test('the query string carries the cursor verbatim', () {
      final cursor = Cursor('2026', 'abc');
      expect(
        PageRequest(limit: 10, after: cursor).toQuery(),
        'limit=10&after=${cursor.encode()}',
      );
      expect(PageRequest(limit: 10).toQuery(), 'limit=10');
    });

    test('a page knows whether it is the last one', () {
      expect(const Page<int>(<int>[], null).isLast, isTrue);
      expect(Page<int>(const <int>[1], Cursor('k', 'i')).isLast, isFalse);
    });
  });

  group('signing', () {
    final key = utf8.encode('a-client-installation-key');
    final body = utf8.encode('{"repository":"a/b"}');

    SignedRequest signed() => signRequest(
          keyId: 'inst_abc',
          key: key,
          method: 'post',
          path: '/v1/runs',
          query: '?limit=10',
          timestamp: now,
          body: body,
        );

    test('the canonical string has exactly six lines in a fixed order', () {
      final canonical = canonicalString(
        method: 'GET',
        path: '/v1/runs',
        query: '',
        timestamp: now,
        body: const <int>[],
      );
      final lines = canonical.split('\n');
      expect(lines.length, 6);
      expect(lines[0], signingScheme);
      expect(lines[1], 'GET');
      expect(lines[2], '/v1/runs');
      expect(lines[3], '');
      expect(lines[4], '$now');
      expect(
        lines[5],
        'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      );
    });

    test('the method is upper-cased and the query loses its leading mark', () {
      expect(
        canonicalString(
          method: 'post',
          path: '/v1/runs',
          query: '?a=1',
          timestamp: now,
          body: const <int>[],
        ),
        canonicalString(
          method: 'POST',
          path: '/v1/runs',
          query: 'a=1',
          timestamp: now,
          body: const <int>[],
        ),
      );
    });

    test('a signature verifies against its own canonical string', () {
      final request = signed();
      expect(
        verifySignature(
          key: key,
          canonical: request.canonical,
          presented: request.signature,
          timestamp: now,
          now: now,
        ),
        isTrue,
      );
    });

    test('changing any covered field invalidates the signature', () {
      final request = signed();
      final tampered = canonicalString(
        method: 'GET',
        path: '/v1/runs',
        query: '?limit=10',
        timestamp: now,
        body: body,
      );
      expect(
        verifySignature(
          key: key,
          canonical: tampered,
          presented: request.signature,
          timestamp: now,
          now: now,
        ),
        isFalse,
      );
    });

    test('the skew window is symmetric and its edge is inside', () {
      final request = signed();
      final far = now + maxSkewSeconds + 1;
      expect(
        verifySignature(
          key: key,
          canonical: request.canonical,
          presented: request.signature,
          timestamp: now,
          now: far,
        ),
        isFalse,
      );
      expect(
        verifySignature(
          key: key,
          canonical: request.canonical,
          presented: request.signature,
          timestamp: far,
          now: now,
        ),
        isFalse,
      );
      expect(
        verifySignature(
          key: key,
          canonical: request.canonical,
          presented: request.signature,
          timestamp: now,
          now: now + maxSkewSeconds,
        ),
        isTrue,
      );
    });

    test('an empty key, key id or newline field is refused', () {
      expect(
        () => signRequest(
          keyId: 'id',
          key: const <int>[],
          method: 'GET',
          path: '/',
          timestamp: now,
        ),
        throwsA(isA<InvalidSigningKey>()),
      );
      expect(
        () => signRequest(
          keyId: '',
          key: key,
          method: 'GET',
          path: '/',
          timestamp: now,
        ),
        throwsA(isA<InvalidSigningKey>()),
      );
      expect(
        () => signRequest(
          keyId: 'id',
          key: key,
          method: 'GET',
          path: '/a\nGET\n/b',
          timestamp: now,
        ),
        throwsA(isA<InvalidSigningKey>()),
      );
    });

    test('signatures are lowercase hex of the right length', () {
      final request = signed();
      expect(request.signature.length, 64);
      expect(RegExp(r'^[0-9a-f]{64}$').hasMatch(request.signature), isTrue);
    });

    test('the headers are the three the server reads', () {
      final headers = signed().headers;
      expect(headers.keys.toSet(),
          <String>{keyIdHeader, timestampHeader, signatureHeader});
      expect(headers[timestampHeader], '$now');
    });

    test('the hex comparison rejects a length mismatch', () {
      expect(timingSafeEqualHex('abcd', 'abcd'), isTrue);
      expect(timingSafeEqualHex('abcd', 'abce'), isFalse);
      expect(timingSafeEqualHex('abcd', 'abcde'), isFalse);
    });
  });

  group('onboarding', () {
    test('the org happy path is reachable one step at a time', () {
      var state = OrgOnboardingState.created;
      var steps = 0;
      while (true) {
        final next = state.nextStep;
        if (next == null) {
          break;
        }
        state = state.transition(next);
        steps += 1;
        expect(steps, lessThan(10), reason: 'the machine has a cycle');
        if (state == OrgOnboardingState.active) {
          break;
        }
      }
      expect(state, OrgOnboardingState.active);
      expect(steps, 6);
    });

    test('skipping billing is rejected', () {
      expect(
        () => OrgOnboardingState.created
            .transition(OrgOnboardingState.active),
        throwsA(isA<IllegalTransition>()),
      );
    });

    test('suspension is never primary but is always reachable', () {
      for (final state in OrgOnboardingState.values) {
        expect(state.nextStep, isNot(OrgOnboardingState.suspended));
        if (state != OrgOnboardingState.suspended) {
          expect(state.successors, contains(OrgOnboardingState.suspended));
        }
      }
    });

    test('progress is monotonic and zero when suspended', () {
      const path = <OrgOnboardingState>[
        OrgOnboardingState.created,
        OrgOnboardingState.verifyingEmail,
        OrgOnboardingState.awaitingBilling,
        OrgOnboardingState.invitingMembers,
        OrgOnboardingState.connectingRepository,
        OrgOnboardingState.firstRunPending,
        OrgOnboardingState.active,
      ];
      for (var index = 1; index < path.length; index += 1) {
        expect(
          path[index - 1].progressPermille < path[index].progressPermille,
          isTrue,
          reason: '${path[index - 1]} -> ${path[index]}',
        );
      }
      expect(OrgOnboardingState.suspended.progressPermille, 0);
      expect(OrgOnboardingState.active.progressPermille, 1000);
    });

    test('the user machine forks at the account-kind choice', () {
      expect(UserOnboardingState.choosingAccountKind.isFork, isTrue);
      expect(UserOnboardingState.registered.isFork, isFalse);
      expect(
        UserOnboardingState.choosingAccountKind.successors,
        containsAll(<UserOnboardingState>[
          UserOnboardingState.joiningOrg,
          UserOnboardingState.connectingRepository,
        ]),
      );
    });

    test('an optimistic move renders now and keeps the confirmed state', () {
      const start = Optimistic<OrgOnboardingState>(OrgOnboardingState.created);
      expect(start.isPending, isFalse);
      final moved = advanceOrg(start, OrgOnboardingState.verifyingEmail);
      expect(moved.isPending, isTrue);
      expect(moved.displayed, OrgOnboardingState.verifyingEmail);
      expect(moved.confirmed, OrgOnboardingState.created);
      // Immutable: the original is untouched.
      expect(start.displayed, OrgOnboardingState.created);
    });

    test('an optimistic move along a missing edge throws', () {
      const start = Optimistic<OrgOnboardingState>(OrgOnboardingState.created);
      expect(
        () => advanceOrg(start, OrgOnboardingState.active),
        throwsA(isA<IllegalTransition>()),
      );
    });

    test('reconciling takes the server word and rollback restores', () {
      final moved = advanceOrg(
        const Optimistic<OrgOnboardingState>(OrgOnboardingState.created),
        OrgOnboardingState.verifyingEmail,
      );
      expect(
        moved.reconcile(OrgOnboardingState.suspended).displayed,
        OrgOnboardingState.suspended,
      );
      expect(moved.rollback().displayed, OrgOnboardingState.created);
    });

    test('a pending move chains from the displayed state', () {
      final first = advanceOrg(
        const Optimistic<OrgOnboardingState>(OrgOnboardingState.created),
        OrgOnboardingState.verifyingEmail,
      );
      final second = advanceOrg(first, OrgOnboardingState.awaitingBilling);
      expect(second.displayed, OrgOnboardingState.awaitingBilling);
      expect(second.confirmed, OrgOnboardingState.created);
    });
  });

  group('resources', () {
    test('a well-formed run parses and defaults its optional fields', () {
      final run = RunResource.fromJson(runJson());
      expect(run.state, 'running');
      expect(run.conclusion, isNull);
      expect(run.jobs, isEmpty);
      expect(run.isInFlight, isTrue);
    });

    test('an unknown state is preserved and keeps the client polling', () {
      final run = RunResource.fromJson(runJson({'state': 'quarantined'}));
      expect(run.state, 'quarantined');
      expect(run.isInFlight, isTrue);
    });

    test('a terminal run is not in flight', () {
      final run = RunResource.fromJson(
        runJson({'state': 'completed', 'conclusion': 'success'}),
      );
      expect(run.isInFlight, isFalse);
      expect(run.conclusion, 'success');
    });

    test('a revision that is not a lowercase commit sha is rejected', () {
      for (final bad in <String>[
        'main',
        '0123456789ABCDEF0123456789ABCDEF01234567',
        '0123456789abcdef0123456789abcdef0123456',
      ]) {
        expect(
          () => RunResource.fromJson(runJson({'revision': bad})),
          throwsA(isA<MalformedResponse>()),
          reason: bad,
        );
      }
    });

    test('nested jobs parse and a bad job fails the whole run', () {
      final run = RunResource.fromJson(runJson({
        'jobs': <Object?>[
          <String, Object?>{
            'id': 'j1',
            'name': 'test',
            'ordinal': 0,
            'profile': 'rust-verify',
            'state': 'running',
            'needs': <Object?>['build'],
            'logOffset': 4096,
          },
        ],
      }));
      expect(run.jobs.single.logOffset, 4096);
      expect(run.jobs.single.needs, <String>['build']);

      expect(
        () => RunResource.fromJson(runJson({
          'jobs': <Object?>[
            <String, Object?>{'id': 'j1'},
          ],
        })),
        throwsA(isA<MalformedResponse>()),
      );
    });

    test('worker capacity never goes negative', () {
      const worker = WorkerResource(
        id: 'w1',
        name: 'arm-1',
        pool: 'shared',
        architecture: 'arm64',
        operatingSystem: 'linux',
        maxConcurrency: 2,
        activeLeases: 5,
      );
      expect(worker.freeCapacity, 0);
    });
  });

  group('streams', () {
    test('distinctRunChanges collapses identical snapshots', () async {
      final run = RunResource.fromJson(runJson());
      final moved = RunResource.fromJson(runJson({'state': 'completed', 'conclusion': 'success'}));
      final emitted = await distinctRunChanges(
        Stream<RunResource>.fromIterable(<RunResource>[run, run, moved, moved]),
      ).toList();
      expect(emitted.length, 2);
      expect(emitted.last.state, 'completed');
    });

    test('untilTerminal emits the terminal snapshot and then closes', () async {
      final running = RunResource.fromJson(runJson());
      final done = RunResource.fromJson(
        runJson({'state': 'completed', 'conclusion': 'success'}),
      );
      final never = RunResource.fromJson(runJson({'id': 'r2'}));
      final emitted = await untilTerminal(
        Stream<RunResource>.fromIterable(<RunResource>[running, done, never]),
      ).toList();
      expect(emitted.length, 2);
      expect(emitted.last.state, 'completed');
    });

    test('withLogOffsets accumulates the resume offset', () async {
      final pairs = await withLogOffsets(
        Stream<String>.fromIterable(<String>['abc', 'de']),
        startOffset: 100,
      ).toList();
      expect(pairs.map((p) => p.offset).toList(), <int>[100, 103]);
      expect(pairs.last.chunk, 'de');
    });
  });
}
