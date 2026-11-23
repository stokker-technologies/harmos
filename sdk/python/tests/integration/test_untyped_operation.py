# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Untyped Calls Share Child Correlation and Preserve Host Failures."""

# === Imports === #

# Standard Library Imports
import json
from unittest import IsolatedAsyncioTestCase

# Third Party Imports
# Local Imports
from harmos import CLASSIFICATION_INVALID, CLASSIFICATION_REFUSED, OperationError, OperationPolicy, OperationRetry, model
from harmos._generated.proto.plugin.v1.instance_pb2 import InstanceInvokeResponse
from harmos._generated.proto.plugin.v1.invocation_pb2 import ChildRequest, ChildResponse, Request
from harmos._invocation import Context

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Output:
    def __init__(self, failure: model.Failure | None = None) -> None:
        self.failure = failure
        self.context = Context(Request(), "fixture", {}, self)
        self.requests: list[ChildRequest] = []

    async def put(self, frame: object) -> None:
        if not isinstance(frame, InstanceInvokeResponse):
            raise TypeError("Write Child: Supply an Invocation Frame")
        child = frame.child
        self.requests.append(child)
        reply = ChildResponse(call=child.call)
        if self.failure is not None:
            reply.failure.CopyFrom(self.failure)
        else:
            reply.payload = child.json.encode()
        await self.context._reply(reply)


class UntypedOperation(IsolatedAsyncioTestCase):
    async def test_json_payload_default_and_policy_use_one_child_carrier(self) -> None:
        output = Output()
        policy = OperationPolicy()
        self.assertEqual(await output.context.operation("fixture.echo", {"radio": "wifi"}, policy=policy), {"radio": "wifi"})
        self.assertEqual(await output.context.operation("fixture.echo"), {})
        self.assertFalse(hasattr(output.context, "call"))
        self.assertEqual([child.call for child in output.requests], [1, 2])
        for child in output.requests:
            self.assertEqual(child.operation, "fixture.echo")
            self.assertTrue(child.HasField("json"))
            self.assertEqual(child.payload, b"")
            self.assertEqual(child.fingerprint, b"")
        self.assertEqual(json.loads(output.requests[0].json), {"radio": "wifi"})
        self.assertTrue(output.requests[0].HasField("policy"))
        self.assertFalse(output.requests[1].HasField("policy"))

    async def test_host_failure_classification_and_text_are_preserved(self) -> None:
        for classification, reason in [
            (CLASSIFICATION_REFUSED, "No Plugin Serves This Operation; Install or Enable the Plugin That Provides It"),
            (CLASSIFICATION_INVALID, 'Field "extra" Is Not in Its Request; Expected radio, state'),
            (CLASSIFICATION_INVALID, 'Field "radio" Must Be String'),
            (CLASSIFICATION_INVALID, 'Field "state" Is Required'),
        ]:
            failure = model.Failure(classification=classification, message=f"Call fixture.echo: {reason}", operation="fixture.echo")
            output = Output(failure)
            with self.assertRaises(OperationError) as raised:
                await output.context.operation("fixture.echo", {})
            self.assertEqual(raised.exception.value, failure)

    async def test_operation_errors_expose_all_kinds_and_retry_evidence(self) -> None:
        for kind in ("transient", "permanent", "refused", "invalid", "cancelled"):
            failure = OperationError(
                kind,
                code="fixture",
                message="Apply Failed",
                action="Choose Another Input",
                operation="fixture.echo",
                retry=OperationRetry(retryable=kind == "transient", attempts=2, max_attempts=3, next_after_s=0.5),
            )
            output = Output(failure.value)
            with self.assertRaises(OperationError) as raised:
                await output.context.operation("fixture.echo", {})
            result = raised.exception.to_dict()
            self.assertEqual(str(raised.exception), "Apply Failed; Choose Another Input")
            self.assertEqual(result["kind"], kind)
            self.assertEqual(result["message"], "Apply Failed")
            self.assertEqual(result["action"], "Choose Another Input")
            self.assertEqual(result["retry"], {"retryable": kind == "transient", "attempts": 2, "max_attempts": 3, "next_after_s": 0.5})
        self.assertEqual(str(OperationError(message="Apply Failed")), "Apply Failed")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
