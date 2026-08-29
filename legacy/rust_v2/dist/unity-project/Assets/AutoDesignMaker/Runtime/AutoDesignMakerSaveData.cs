using System;

namespace AutoDesignMaker.Runtime
{
    [Serializable]
    public sealed class AutoDesignMakerSaveData
    {
        public string target_id;
        public string build_profile;
        public float session_seconds;
        public int autosave_frame;
        public string last_input_axis;
        public string active_mechanic;
        public int mechanic_index;
        public string[] pipeline_artifacts;
    }
}
