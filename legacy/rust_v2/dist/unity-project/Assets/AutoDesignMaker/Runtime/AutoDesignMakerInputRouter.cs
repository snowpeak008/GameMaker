using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerInputRouter : MonoBehaviour
    {
        public Vector2 MoveAxis { get; private set; }
        public bool ConfirmPressed { get; private set; }
        public bool CancelPressed { get; private set; }
        public string AxisText => MoveAxis.x.ToString("0.00") + "," + MoveAxis.y.ToString("0.00");

        private void Update()
        {
            MoveAxis = new Vector2(Input.GetAxisRaw("Horizontal"), Input.GetAxisRaw("Vertical"));
            ConfirmPressed = Input.GetKeyDown(KeyCode.Space) || Input.GetKeyDown(KeyCode.Return);
            CancelPressed = Input.GetKeyDown(KeyCode.Escape);
        }
    }
}
