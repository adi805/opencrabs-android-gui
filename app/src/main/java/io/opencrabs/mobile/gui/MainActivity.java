package io.opencrabs.mobile.gui;

import com.google.androidgamesdk.GameActivity;

import android.os.Bundle;

public class MainActivity extends GameActivity {
    static {
        // Must match [lib].name from Cargo.toml -> liboc_gui.so
        System.loadLibrary("oc_gui");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
    }
}
