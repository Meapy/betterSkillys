package kabam.rotmg.storage {
import com.company.assembleegameclient.game.GameSprite;
import com.company.assembleegameclient.objects.GameObject;
import com.company.assembleegameclient.parameters.Parameters;
import com.company.assembleegameclient.ui.TextBox;
import com.company.assembleegameclient.ui.panels.ButtonPanel;

import flash.events.KeyboardEvent;
import flash.events.MouseEvent;

public class PotionsPanel extends ButtonPanel {

    public function PotionsPanel(gs:GameSprite) {
        super(gs, "Potion Storage", "View");
    }

    // one storage window at a time: opening it again (button or interact key) closes it, so windows
    // can't pile up behind each other
    private static var open_:PotionsView;

    private function toggle():void {
        if (open_ != null && open_.parent != null) {
            open_.onClose(null);
            open_ = null;
            return;
        }
        open_ = new PotionsView(this.gs_);
        this.gs_.scaledLayer.addChild(open_);
    }

    override protected function onButtonClick(evt:MouseEvent):void {
        this.toggle();
    }

    override protected function onKeyDown(evt:KeyboardEvent):void {
        if (evt.keyCode == Parameters.data_.interact && !TextBox.isInputtingText) {
            this.toggle();
        }
    }

}
}
