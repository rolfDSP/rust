"""Train a small classifier in PyTorch and export it for the Rust program.

Writes two files to ../model:
  spiral.onnx        the network, self-contained: graph plus weights
  test_vectors.json  held-out points with PyTorch's own outputs, so that Rust
                     can check that it gets the same numbers
"""

import json
import math
from pathlib import Path

import onnx
import torch
from torch import nn

MODEL_DIR = Path(__file__).resolve().parent.parent / "model"
ONNX_PATH = MODEL_DIR / "spiral.onnx"
VECTORS_PATH = MODEL_DIR / "test_vectors.json"


class SpiralNet(nn.Module):
    def __init__(self):
        super().__init__()
        self.layers = nn.Sequential(
            nn.Linear(2, 64),
            nn.ReLU(),
            nn.Linear(64, 64),
            nn.ReLU(),
            nn.Linear(64, 2),
        )

    def forward(self, points):
        return self.layers(points)


def make_spirals(points_per_class):
    """Two interleaved spirals: not separable by a straight line."""
    radius = torch.linspace(0.1, 1.0, points_per_class)
    angle = radius * 3.0 * math.tau

    def spiral(offset):
        theta = angle + offset
        return torch.stack([radius * theta.cos(), radius * theta.sin()], dim=1)

    clean = torch.cat([spiral(0.0), spiral(math.pi)])
    points = clean + torch.randn(2 * points_per_class, 2) * 0.02
    labels = torch.cat(
        [torch.zeros(points_per_class), torch.ones(points_per_class)]
    ).long()
    return points, labels


def main():
    torch.manual_seed(42)
    MODEL_DIR.mkdir(exist_ok=True)

    # Shuffle, then hold back 20% to test on points never trained on.
    points, labels = make_spirals(1000)
    order = torch.randperm(len(points))
    points, labels = points[order], labels[order]
    train_size = len(points) * 8 // 10
    train_x, test_x = points[:train_size], points[train_size:]
    train_y, test_y = labels[:train_size], labels[train_size:]

    model = SpiralNet()
    optimizer = torch.optim.Adam(model.parameters(), lr=5e-3)
    loss_fn = nn.CrossEntropyLoss()

    for epoch in range(1, 401):
        optimizer.zero_grad()
        loss = loss_fn(model(train_x), train_y)
        loss.backward()
        optimizer.step()
        if epoch % 100 == 0:
            print(f"epoch {epoch:3}  loss {loss.item():.4f}")

    # Layers such as dropout behave differently in training and inference, and
    # the export records whichever mode is active. Always switch first.
    model.eval()
    with torch.no_grad():
        test_logits = model(test_x)
    accuracy = (test_logits.argmax(dim=1) == test_y).float().mean().item()
    print(f"PyTorch test accuracy: {100 * accuracy:.1f}%")

    # The export traces the model with this example input. Without
    # `dynamic_shapes` the batch size of the example (1) would be fixed in the
    # graph, and the model would reject any other batch size.
    onnx_program = torch.onnx.export(
        model,
        (torch.zeros(1, 2),),
        input_names=["points"],
        output_names=["logits"],
        dynamic_shapes={"points": {0: torch.export.Dim("batch")}},
    )
    # `external_data=False` stores the weights inside the .onnx file instead of
    # in a separate .onnx.data file next to it.
    onnx_program.save(ONNX_PATH, external_data=False)
    onnx.checker.check_model(onnx.load(ONNX_PATH), full_check=True)
    print(f"Wrote {ONNX_PATH.name} ({ONNX_PATH.stat().st_size} bytes)")

    VECTORS_PATH.write_text(
        json.dumps(
            {
                "points": test_x.tolist(),
                "labels": test_y.tolist(),
                "logits": test_logits.tolist(),
            }
        )
    )
    print(f"Wrote {VECTORS_PATH.name} ({len(test_x)} points)")


if __name__ == "__main__":
    main()
