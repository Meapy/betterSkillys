package com.company.assembleegameclient.game
{
   import flash.display.Shape;
   import flash.events.Event;
   import flash.filesystem.File;
   import flash.filesystem.FileMode;
   import flash.filesystem.FileStream;
   import flash.sampler.*;
   import flash.utils.getTimer;

   import com.company.assembleegameclient.engine3d.Face3D;
   import com.company.assembleegameclient.map.Map;
   import com.company.assembleegameclient.parameters.Parameters;
   import kabam.rotmg.stage3D.Renderer;

   // Test build only (tools/testclient/build_test_client.py copies it in and hooks it up).
   // Lines in <app storage>/autochat.txt are read after entering the game; the file is consumed.
   // A line starting with "#" is a local switch (see local()), anything else is sent as chat.
   public class TestHook
   {
      private static var lines:Array = null;
      private static var nextAt:int = 0;
      private static var checkAt:int = 0;

      // 1: skip GameSprite's frame update, 2: skip only the map draw, 3: draw but don't present
      public static var skip:int = 0;
      // 1: move a 4 px square every frame (a display list change and nothing else)
      public static var poke:int = 0;
      private static var pokeShape:Shape = null;
      // "#sample N": objects created over the next N frames, by type and creating code
      private static var sampleFrames:int = 0;
      private static var sampledFrames:int = 0;

      public static function next():String
      {
         var now:int = getTimer();
         if (lines == null)
         {
            lines = [];
            nextAt = now + 4000;
            checkAt = now + 4000;
         }
         if (lines.length == 0 && now >= checkAt)
         {
            checkAt = now + 2000;
            try
            {
               var f:File = File.applicationStorageDirectory.resolvePath("autochat.txt");
               if (f.exists)
               {
                  var s:FileStream = new FileStream();
                  s.open(f, FileMode.READ);
                  var text:String = s.readUTFBytes(s.bytesAvailable);
                  s.close();
                  f.deleteFile();
                  for each (var l:String in text.split("\n"))
                  {
                     l = l.replace(/\r/g, "");
                     if (l.length > 0)
                        lines.push(l);
                  }
               }
            }
            catch (e:Error) { }
         }
         if (lines.length == 0 || now < nextAt)
            return null;
         nextAt = now + (lines[0].charAt(0) == "#" ? 50 : 3000);
         return lines.shift();
      }

      // On entering the game: statistics on, frame rate cap high enough not to hide anything.
      public static function reset():void
      {
         lines = null;
         Parameters.data_.showStatistics = true;
         Parameters.data_.fps = 360;
      }

      // Runs at the start of every frame.
      public static function tick():void
      {
         if (poke == 1)
         {
            if (pokeShape == null)
            {
               pokeShape = new Shape();
               pokeShape.graphics.beginFill(0xFF0000);
               pokeShape.graphics.drawRect(0, 0, 4, 4);
               WebMain.STAGE.addChild(pokeShape);
            }
            pokeShape.x = (pokeShape.x + 1) % 50;
         }
         sampleTick();
      }

      public static function local(line:String, gs:* = null):Boolean
      {
         if (line.charAt(0) != "#") return false;
         var a:Array = line.substr(1).split(" ");
         var on:Boolean = a[1] != "0";
         switch (a[0])
         {
            // optimisation switches, to compare with and without
            case "batch": Renderer.batching = on; break;
            case "atlas": Renderer.atlasing = on; break;
            case "chunk": Map.chunking = on; break;
            case "cull": Map.culling = on; break;
            case "cache": Face3D.caching = on; break;
            // Stage3D error checking: slow, but shows errors the GPU otherwise skips silently
            case "errors": WebMain.STAGE.stage3Ds[0].context3D.enableErrorChecking = on; break;
            // view
            case "angle": Parameters.data_.cameraAngle = Number(a[1]); break;
            case "mscale": Parameters.data_.mscale = Number(a[1]); Parameters.root.dispatchEvent(new Event(Event.RESIZE)); break;
            case "full": WebMain.STAGE.displayState = on ? "fullScreenInteractive" : "normal"; break;
            case "hud": if (gs != null && gs.hudView != null) gs.hudView.visible = on; break;
            case "mapvis": if (gs != null) gs.map.visible = on; break;
            case "quality": WebMain.STAGE.quality = a[1]; break;
            // profiling
            case "fps": WebMain.STAGE.frameRate = Number(a[1]); break;
            case "skip": skip = int(a[1]); break;
            case "poke": poke = int(a[1]); break;
            case "sample": sampleFrames = int(a[1]); sampledFrames = 0; clearSamples(); startSampling(); break;
         }
         return true;
      }

      // Needs a debug build (the runtime only samples debug SWFs). Writes <app storage>/samples.txt.
      private static function sampleTick():void
      {
         if (sampleFrames <= 0) return;
         sampledFrames++;
         if (--sampleFrames > 0) return;
         pauseSampling();
         var byType:Object = {};
         var bySite:Object = {};
         var total:int = 0;
         for each (var s:* in getSamples())
         {
            if (!(s is NewObjectSample)) continue;
            total++;
            var ty:String = String(NewObjectSample(s).type);
            byType[ty] = (byType[ty] || 0) + 1;
            var st:Array = s.stack;
            var site:String = ty;
            if (st != null)
               for (var i:int = 0; i < Math.min(3, st.length); i++)
                  site += " <- " + StackFrame(st[i]).name + ":" + StackFrame(st[i]).line;
            bySite[site] = (bySite[site] || 0) + 1;
         }
         stopSampling();
         clearSamples();
         var out:String = "objects per frame: " + (total / sampledFrames).toFixed(0) + " over " + sampledFrames + " frames\n\nBY TYPE\n" + top(byType, 40) + "\nBY SITE\n" + top(bySite, 80);
         write("samples.txt", out, FileMode.WRITE);
      }

      private static function top(o:Object, n:int):String
      {
         var a:Array = [];
         for (var k:String in o) a.push({k: k, v: o[k]});
         a.sortOn("v", Array.NUMERIC | Array.DESCENDING);
         var r:String = "";
         for (var i:int = 0; i < Math.min(n, a.length); i++) r += (a[i].v / sampledFrames).toFixed(1) + "  " + a[i].k + "\n";
         return r;
      }

      // Uncaught errors go to <app storage>/errors.txt.
      public static function onError(e:*):void
      {
         try
         {
            var err:* = e.error;
            write("errors.txt", new Date().toString() + " " + (err is Error ? (Error(err).getStackTrace() || Error(err).message) : String(err)) + "\n", FileMode.APPEND);
         }
         catch (x:Error) { }
      }

      private static function write(name:String, text:String, mode:String):void
      {
         var f:File = File.applicationStorageDirectory.resolvePath(name);
         var s:FileStream = new FileStream();
         s.open(f, mode);
         s.writeUTFBytes(text);
         s.close();
      }
   }
}
