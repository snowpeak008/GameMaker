using UnityEngine;

namespace AutoDesignMaker.Generated
{
    public sealed class AutoDesignMakerBootstrap : MonoBehaviour
    {
        public const string TargetId = "windows_desktop_playable";
        public const string BuildProfile = "playable-prototype";

        private void Awake()
        {
            if (GetComponent<AutoDesignMaker.Runtime.AutoDesignMakerRuntimeController>() == null)
            {
                gameObject.AddComponent<AutoDesignMaker.Runtime.AutoDesignMakerRuntimeController>();
            }
        }

        private void Start()
        {
            Debug.Log($"AutoDesignMaker bootstrap mounted for {TargetId} / {BuildProfile}");
        }
    }
}
